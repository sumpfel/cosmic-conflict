//! Geometry on spheres: everything on a planet's surface is a unit direction from its centre,
//! ground units walk along great circles.

use glam::{Quat, Vec3};

/// Angle in radians between two unit vectors.
pub fn angle_between(a: Vec3, b: Vec3) -> f32 {
    libm::acosf(a.dot(b).clamp(-1.0, 1.0))
}

/// Any unit vector perpendicular to `n`.
pub fn any_perpendicular(n: Vec3) -> Vec3 {
    let helper = if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    n.cross(helper).normalize()
}

/// Rotates `dir` towards `goal` along the great circle by at most `max_angle` radians.
/// Returns the new direction and whether the goal was reached.
pub fn step_towards(dir: Vec3, goal: Vec3, max_angle: f32) -> (Vec3, bool) {
    let ang = angle_between(dir, goal);
    if ang <= max_angle || ang < 1e-6 {
        return (goal, true);
    }
    let mut axis = dir.cross(goal);
    if axis.length_squared() < 1e-12 {
        axis = any_perpendicular(dir);
    }
    let q = Quat::from_axis_angle(axis.normalize(), max_angle);
    ((q * dir).normalize(), false)
}

/// A point `angle` radians away from `dir` in compass direction `azimuth` (radians).
pub fn offset(dir: Vec3, azimuth: f32, angle: f32) -> Vec3 {
    let t = any_perpendicular(dir);
    let b = dir.cross(t);
    let side = t * libm::cosf(azimuth) + b * libm::sinf(azimuth);
    (dir * libm::cosf(angle) + side * libm::sinf(angle)).normalize()
}

/// Rotation about the world Y axis built with `libm` (deterministic on every platform).
pub fn rot_y(angle: f32) -> Quat {
    Quat::from_xyzw(0.0, libm::sinf(angle * 0.5), 0.0, libm::cosf(angle * 0.5))
}

/// Intersection of a ray with a sphere: distance along the ray to the first hit in front.
pub fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t0 = -b - s;
    let t1 = -b + s;
    if t0 > 0.0 {
        Some(t0)
    } else if t1 > 0.0 {
        Some(t1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_reaches_the_goal() {
        let mut d = Vec3::Y;
        let goal = Vec3::new(1.0, 0.2, -0.3).normalize();
        let mut steps = 0;
        loop {
            let (n, done) = step_towards(d, goal, 0.05);
            assert!((n.length() - 1.0).abs() < 1e-4);
            d = n;
            steps += 1;
            if done {
                break;
            }
            assert!(steps < 200);
        }
        // Antipodal points work too.
        let (n, _) = step_towards(Vec3::Y, -Vec3::Y, 0.1);
        assert!((angle_between(n, Vec3::Y) - 0.1).abs() < 1e-3);
    }

    #[test]
    fn offset_has_the_right_angle() {
        let d = Vec3::new(0.3, -0.5, 0.8).normalize();
        for az in [0.0, 1.0, 2.5, 4.0] {
            let o = offset(d, az, 0.3);
            assert!((angle_between(d, o) - 0.3).abs() < 1e-4);
        }
    }

    #[test]
    fn ray_hits_sphere() {
        let t = ray_sphere(Vec3::new(0.0, 0.0, -10.0), Vec3::Z, Vec3::ZERO, 2.0).unwrap();
        assert!((t - 8.0).abs() < 1e-4);
        assert!(ray_sphere(Vec3::new(0.0, 5.0, -10.0), Vec3::Z, Vec3::ZERO, 2.0).is_none());
    }
}
