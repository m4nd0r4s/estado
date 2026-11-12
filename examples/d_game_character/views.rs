//! Generated views (spec 6.2): the split borrow of `Alive`.
//!
//! Every leaf of this machine is a unit variant, so there are no `*Mut` views;
//! only the parallel state has fields shared with its regions.

use super::model::{Alive, Anim, Movement, Weapon};

/// The fields of `Alive` without its `#[region]` fields.
///
/// Region handlers get it next to their own data, so the handler of a weapon
/// state can spend `ammo` while the dispatch still holds all three regions.
pub struct AliveFields<'a> {
    /// `Alive::health`.
    pub health: &'a mut u16,
    /// `Alive::ammo`.
    pub ammo: &'a mut u8,
}

impl Alive {
    /// Splits the borrow: the fields, then the regions in declaration order.
    pub fn split(&mut self) -> (AliveFields<'_>, &mut Movement, &mut Weapon, &mut Anim) {
        let Self {
            health,
            ammo,
            movement,
            weapon,
            anim,
        } = self;
        (AliveFields { health, ammo }, movement, weapon, anim)
    }
}
