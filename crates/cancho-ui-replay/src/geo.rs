//! `distance(a, b, default)` (docs/maps.md section 3.1): the great-circle distance in kilometres between two
//! `"lat,lon"` texts, the text `location.current()` answers. The emitted `__cui_distance` is the same function;
//! `tests/distance_corpus.json` is checked against both (to 1e-9 km: the two libraries' `sin` and `asin` may differ in
//! the last digit).

use crate::eval::num_of_text;

/// The mean Earth radius in kilometres (IUGG).
const EARTH_KM: f64 = 6371.0088;

/// A position: `lat,lon` with exactly one comma, each part exactly a number (the grammar of `num`), the latitude in
/// [-90, 90] and the longitude in [-180, 180]. Spaces, a second comma, an empty part and a number out of range are not a position.
pub fn parse_position(text: &str) -> Option<(f64, f64)> {
    let mut parts = text.split(',');
    let (lat, lon) = (parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let lat = num_of_text(lat, f64::NAN);
    let lon = num_of_text(lon, f64::NAN);
    (lat.abs() <= 90.0 && lon.abs() <= 180.0).then_some((lat, lon))
}

/// Haversine, in kilometres; `default` when either text is not a position. Never NaN unless `default` is.
pub fn distance_km(a: &str, b: &str, default: f64) -> f64 {
    match (parse_position(a), parse_position(b)) {
        (Some((la, ga)), Some((lb, gb))) => {
            let (pa, pb) = (la.to_radians(), lb.to_radians());
            let h = ((pb - pa) / 2.0).sin().powi(2)
                + pa.cos() * pb.cos() * ((gb - ga).to_radians() / 2.0).sin().powi(2);
            2.0 * EARTH_KM * h.sqrt().min(1.0).asin()
        }
        _ => default,
    }
}
