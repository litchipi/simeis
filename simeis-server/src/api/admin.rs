use ntex::router::IntoPattern;
use ntex::web::{scope, ServiceConfig};

pub fn configure<T: IntoPattern>(base: T, srv: &mut ServiceConfig) {
    srv.service(scope(base));
}
