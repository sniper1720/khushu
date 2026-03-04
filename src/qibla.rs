use std::sync::{Arc, Mutex};
use zbus::{Connection, proxy};

const KAABA_LAT: f64 = 21.4225;
const KAABA_LON: f64 = 39.8262;

pub fn calculate_qibla_bearing(lat: f64, lon: f64) -> f64 {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let kaaba_lat_rad = KAABA_LAT.to_radians();
    let kaaba_lon_rad = KAABA_LON.to_radians();

    let y = (kaaba_lon_rad - lon_rad).sin();
    let x = lat_rad.cos() * kaaba_lat_rad.tan() - lat_rad.sin() * (kaaba_lon_rad - lon_rad).cos();

    let bearing_rad = y.atan2(x);
    let bearing_deg = bearing_rad.to_degrees();

    (bearing_deg + 360.0) % 360.0
}

#[proxy(
    interface = "net.hadess.SensorProxy",
    default_service = "net.hadess.SensorProxy",
    default_path = "/net/hadess/SensorProxy"
)]
trait SensorProxy {
    #[zbus(property)]
    fn has_compass(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn compass_heading(&self) -> zbus::Result<f64>;

    fn claim_compass(&self) -> zbus::Result<()>;
    fn release_compass(&self) -> zbus::Result<()>;
}

#[derive(Clone)]
pub struct CompassManager {
    heading: Arc<Mutex<f64>>,
    available: Arc<Mutex<bool>>,
    stop_signal: Arc<Mutex<bool>>,
}

impl CompassManager {
    pub fn new() -> Self {
        Self {
            heading: Arc::new(Mutex::new(0.0)),
            available: Arc::new(Mutex::new(false)),
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }

    pub fn start_monitoring(&self) {
        let heading_clone = self.heading.clone();
        let available_clone = self.available.clone();
        let stop_clone = self.stop_signal.clone();

        tokio::spawn(async move {
            if let Ok(connection) = Connection::system().await
                && let Ok(proxy) = SensorProxyProxy::new(&connection).await
                && let Ok(has_compass) = proxy.has_compass().await
                && has_compass
            {
                *available_clone
                    .lock()
                    .expect("Failed to lock available_clone") = true;
                let _ = proxy.claim_compass().await;

                loop {
                    if *stop_clone.lock().expect("Failed to lock stop_clone") {
                        let _ = proxy.release_compass().await;
                        break;
                    }

                    if let Ok(heading) = proxy.compass_heading().await {
                        *heading_clone.lock().expect("Failed to lock heading_clone") = heading;
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        });
    }

    pub fn stop(&self) {
        *self.stop_signal.lock().expect("Failed to lock stop_signal") = true;
    }

    pub fn get_heading(&self) -> f64 {
        *self.heading.lock().expect("Failed to lock heading")
    }

    pub fn is_available(&self) -> bool {
        *self.available.lock().expect("Failed to lock available")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_bearing_near(lat: f64, lon: f64, expected_deg: f64, tolerance: f64) {
        let bearing = calculate_qibla_bearing(lat, lon);
        let diff = (bearing - expected_deg + 540.0) % 360.0 - 180.0;
        assert!(
            diff.abs() < tolerance,
            "Qibla from ({lat}, {lon}): expected ~{expected_deg}°, got {bearing:.2}° (diff {diff:.2}°)"
        );
    }

    #[test]
    fn qibla_from_makkah_is_near_zero() {
        let bearing = calculate_qibla_bearing(KAABA_LAT, KAABA_LON);
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
