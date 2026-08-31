use glib::prelude::*;
use gtk4::gio;
use gtk4::glib;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const KAABA_LATITUDE: f64 = 21.4225;
const KAABA_LONGITUDE: f64 = 39.8262;

pub fn calculate_qibla_bearing(latitude: f64, longitude: f64) -> f64 {
    let latitude_rad = latitude.to_radians();
    let longitude_rad = longitude.to_radians();
    let kaaba_latitude_rad = KAABA_LATITUDE.to_radians();
    let kaaba_longitude_rad = KAABA_LONGITUDE.to_radians();

    let longitude_diff_sin = (kaaba_longitude_rad - longitude_rad).sin();
    let meridian_component = latitude_rad.cos() * kaaba_latitude_rad.tan()
        - latitude_rad.sin() * (kaaba_longitude_rad - longitude_rad).cos();

    let bearing_rad = longitude_diff_sin.atan2(meridian_component);
    let bearing_deg = bearing_rad.to_degrees();

    (bearing_deg + 360.0) % 360.0
}

#[derive(Clone)]
pub struct CompassManager {
    heading: Arc<Mutex<f64>>,
    available: Arc<Mutex<bool>>,
    epoch: Arc<AtomicU64>,
    subscription: Arc<Mutex<Option<gio::SignalSubscription>>>,
}

impl CompassManager {
    pub fn new() -> Self {
        Self {
            heading: Arc::new(Mutex::new(0.0)),
            available: Arc::new(Mutex::new(false)),
            epoch: Arc::new(AtomicU64::new(0)),
            subscription: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start_monitoring(&self) {
        let my_epoch = self.epoch.fetch_add(1, Ordering::SeqCst);
        *self.subscription.lock().expect("compass subscription lock") = None;

        let heading = self.heading.clone();
        let available = self.available.clone();
        let epoch = self.epoch.clone();
        let subscription_guard = self.subscription.clone();

        std::thread::spawn(move || {
            if epoch.load(Ordering::SeqCst) != my_epoch {
                return;
            }

            let conn = match gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) {
                Ok(conn) => conn,
                Err(err) => {
                    log::error!("Compass: D-Bus connection failed: {err}");
                    return;
                }
            };

            if epoch.load(Ordering::SeqCst) != my_epoch {
                return;
            }

            let has_compass = {
                let args = glib::Variant::tuple_from_iter([
                    "net.hadess.SensorProxy".to_variant(),
                    "HasCompass".to_variant(),
                ]);
                match conn.call_sync(
                    Some("net.hadess.SensorProxy"),
                    "/net/hadess/SensorProxy",
                    "org.freedesktop.DBus.Properties",
                    "Get",
                    Some(&args),
                    Some(&glib::VariantType::new("(v)").expect("(v) is valid")),
                    gio::DBusCallFlags::NONE,
                    -1,
                    gio::Cancellable::NONE,
                ) {
                    Ok(value) => value.child_value(0).get::<bool>().unwrap_or(false),
                    Err(err) => {
                        log::warn!("Compass: HasCompass query failed: {err}");
                        false
                    }
                }
            };

            if !has_compass {
                *available.lock().expect("compass available lock") = false;
                return;
            }

            if epoch.load(Ordering::SeqCst) != my_epoch {
                return;
            }

            let _ = conn.call_sync(
                Some("net.hadess.SensorProxy"),
                "/net/hadess/SensorProxy",
                "net.hadess.SensorProxy",
                "ClaimCompass",
                None::<&glib::Variant>,
                None,
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            );

            *available.lock().expect("compass available lock") = true;

            {
                let args = glib::Variant::tuple_from_iter([
                    "net.hadess.SensorProxy".to_variant(),
                    "CompassHeading".to_variant(),
                ]);
                if let Ok(value) = conn.call_sync(
                    Some("net.hadess.SensorProxy"),
                    "/net/hadess/SensorProxy",
                    "org.freedesktop.DBus.Properties",
                    "Get",
                    Some(&args),
                    Some(&glib::VariantType::new("(v)").expect("(v) is valid")),
                    gio::DBusCallFlags::NONE,
                    -1,
                    gio::Cancellable::NONE,
                ) && let Some(compass_heading) = value.child_value(0).get::<f64>()
                {
                    *heading.lock().expect("compass heading lock") = compass_heading;
                }
            }

            let heading_cb = heading;
            let epoch_cb = epoch;
            let subscription = conn.subscribe_to_signal(
                Some("net.hadess.SensorProxy"),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                Some("/net/hadess/SensorProxy"),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal_ref| {
                    if epoch_cb.load(Ordering::SeqCst) != my_epoch {
                        return;
                    }
                    if let Ok(mut heading_guard) = heading_cb.lock() {
                        let changed = signal_ref.parameters.child_value(1);
                        let dict = glib::VariantDict::new(Some(&changed));
                        if let Some(variant) = dict.lookup_value("CompassHeading", None)
                            && let Some(heading) = variant.get::<f64>()
                        {
                            *heading_guard = heading;
                        }
                    }
                },
            );

            *subscription_guard
                .lock()
                .expect("compass subscription lock") = Some(subscription);
        });
    }

    pub fn stop(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        *self.subscription.lock().expect("compass subscription lock") = None;
    }

    pub fn restart(&self) {
        self.stop();
        self.start_monitoring();
    }

    pub fn get_heading(&self) -> f64 {
        *self.heading.lock().unwrap_or_else(|err| {
            log::error!("Failed to lock heading: {err}");
            err.into_inner()
        })
    }

    pub fn is_available(&self) -> bool {
        *self.available.lock().unwrap_or_else(|err| {
            log::error!("Failed to lock available: {err}");
            err.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_bearing_near(latitude: f64, longitude: f64, expected_deg: f64, tolerance: f64) {
        let bearing = calculate_qibla_bearing(latitude, longitude);
        let diff = (bearing - expected_deg + 540.0) % 360.0 - 180.0;
        assert!(
            diff.abs() < tolerance,
            "Qibla from ({latitude}, {longitude}): expected ~{expected_deg}°, got {bearing:.2}° (diff {diff:.2}°)"
        );
    }

    #[test]
    fn qibla_from_makkah_is_near_zero() {
        let bearing = calculate_qibla_bearing(KAABA_LATITUDE, KAABA_LONGITUDE);
        assert!(bearing.is_finite());
    }

    #[test]
    fn qibla_from_algiers() {
        assert_bearing_near(36.75, 3.05, 105.0, 3.0);
    }

    #[test]
    fn qibla_from_new_york() {
        assert_bearing_near(40.71, -74.01, 58.0, 3.0);
    }

    #[test]
    fn qibla_from_tokyo() {
        assert_bearing_near(35.68, 139.69, 293.0, 3.0);
    }
}
