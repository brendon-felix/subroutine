/// Ease out with quadratic function
use std::f32::consts::PI;

pub fn step_start(t: f32) -> f32 {
    match t {
        t if t < 0.25 => 0.0,
        t if t < 0.75 => (t - 0.25) * 2.0,
        _ => 1.0,
    }
}

pub fn reverse(t: f32) -> f32 {
    1.0 - t
}

/// Ease in with quadratic function
pub fn ease_in(t: f32) -> f32 {
    t * t
}

/// Ease out with quadratic function
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(2)
}

/// The Quint ease-out function, which starts quickly and decelerates to a stop
pub fn ease_out_quint() -> impl Fn(f32) -> f32 {
    move |delta| 1.0 - (1.0 - delta).powi(5)
}

/// Ease in and out with quadratic function
pub fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        let x = -2.0 * t + 2.0;
        1.0 - x * x / 2.0
    }
}

/// Step start ease
pub fn ease_step_start(t: f32) -> f32 {
    ease_in_out(step_start(t))
}

/// Cubic ease in
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

/// Cubic ease out
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease in and out
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn ease_out_cubic_bezier(t: f32) -> f32 {
    // Cubic Bezier control points for ease-out
    let p0 = 0.0;
    let p1 = 0.33;
    let p2 = 0.67;
    let p3 = 1.0;

    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    let result = uuu * p0 + 3.0 * uu * t * p1 + 3.0 * u * tt * p2 + ttt * p3;
    result.clamp(0.0, 1.0)
}

/// Elastic ease in
pub fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        let c4 = (2.0 * PI) / 3.0;
        let result = -(2.0_f32.powf(10.0 * (t - 1.0))) * ((t - 1.0) * c4 - PI / 2.0).sin();
        result.clamp(0.0, 1.0)
    }
}

/// Elastic ease out
pub fn ease_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        let c4 = (2.0 * PI) / 3.0;
        let result = 2.0_f32.powf(-10.0 * t) * (t * c4 - PI / 2.0).sin() + 1.0;
        result.clamp(0.0, 1.0)
    }
}

/// Elastic ease in and out
pub fn ease_in_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else if t < 0.5 {
        let c5 = (2.0 * PI) / 4.5;
        let result = -(2.0_f32.powf(20.0 * t - 10.0)) * ((20.0 * t - 11.125) * c5).sin() / 2.0;
        result.clamp(0.0, 1.0)
    } else {
        let c5 = (2.0 * PI) / 4.5;
        let result = 2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin() / 2.0 + 1.0;
        result.clamp(0.0, 1.0)
    }
}

/// Bounce ease out
pub fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;

    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// Bounce ease in
pub fn ease_in_bounce(t: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - t)
}

/// Bounce ease in and out
pub fn ease_in_out_bounce(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
    }
}

/// Back ease in
pub fn ease_in_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;

    let result = c3 * t * t * t - c1 * t * t;
    result.clamp(0.0, 1.0)
}

/// Back ease out
pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;

    let result = 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2);
    result.clamp(0.0, 1.0)
}

/// Back ease in and out
pub fn ease_in_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c2 = c1 * 1.525;

    let result = if t < 0.5 {
        ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
    } else {
        ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
    };
    result.clamp(0.0, 1.0)
}

/// Creates a pulsating easing function between min and max values
pub fn pulsating_between(min: f32, max: f32) -> impl Fn(f32) -> f32 {
    move |t: f32| {
        let normalized = (t * 2.0 * PI).sin() * 0.5 + 0.5;
        min + normalized * (max - min)
    }
}

/// Creates a bounce easing function with a custom inner easing
pub fn bounce<F>(inner_easing: F) -> impl Fn(f32) -> f32
where
    F: Fn(f32) -> f32,
{
    move |t: f32| -> f32 {
        let bounced_t = if t < 0.5 {
            ease_out_bounce(t * 2.0) * 0.5
        } else {
            0.5 + ease_in_bounce((t - 0.5) * 2.0) * 0.5
        };
        inner_easing(bounced_t)
    }
}

// /// Creates a custom spring ease function
// pub fn spring(tension: f32, friction: f32) -> impl Fn(f32) -> f32 {
//     move |t: f32| -> f32 {
//         let result =
//         result.clamp(0.0, 1.0)
//     }
// }

// /// Creates a custom ease function that oscillates
// pub fn oscillate(frequency: f32, decay: f32) -> impl Fn(f32) -> f32 {
//     move |t: f32| -> f32 {
//         result.clamp(0.0, 1.0)
//     }
// }

// /// Standard pulsing animation (0.3 to 1.0)
// pub fn standard_pulse(t: f32) -> f32 {
//     0.3 + 0.7 * ((t * 2.0 * PI).sin() * 0.5 + 0.5)
// }

// /// Gentle spring animation
// pub fn standard_spring(t: f32) -> f32 {
//     spring(6.0, 3.0)(t)
// }

// /// Oscillating ease with standard parameters
// pub fn standard_oscillate(t: f32) -> f32 {
//     oscillate(6.0, 3.0)(t)
// }
