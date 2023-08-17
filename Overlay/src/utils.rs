use bevy::prelude::Vec3;

pub trait ToGw2Coordinate {
    fn as_gw2_coordinate(&self) -> Self;
    fn to_gw2_coordinate(&mut self);
}

impl ToGw2Coordinate for Vec3 {
    fn as_gw2_coordinate(&self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: -self.z,
        }
    }

    fn to_gw2_coordinate(&mut self) {
        self.z *= -1.0;
    }
}
