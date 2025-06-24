use sails_rs::prelude::*;
use alloc::collections::BTreeMap;

#[derive(Encode, TypeInfo, Clone)]
pub struct Transfer { pub from: ActorId, pub to: ActorId, pub amount: u128 }

#[derive(Encode, TypeInfo)]
pub enum VftEvent {
    Minted(ActorId, u128),
    Burned(ActorId, u128),
    Transferred(Transfer),
}

#[derive(Clone)]
pub struct VftService {
    balances: BTreeMap<ActorId, u128>,
}

#[service(events = VftEvent)]
impl VftService {
    pub fn new() -> Self {
        Self { balances: BTreeMap::new() }
    }

    pub fn mint(&mut self, to: ActorId, amount: u128) {
        assert!(amount > 0, "Mint > 0");
        *self.balances.entry(to).or_default() += amount;
        let _ = self.emit_event(VftEvent::Minted(to, amount));
    }

    pub fn burn(&mut self, from: ActorId, amount: u128) {
        let bal = self.balances.entry(from).or_default();
        assert!(*bal >= amount, "Insufficient balance");
        *bal -= amount;
        let _ = self.emit_event(VftEvent::Burned(from, amount));
    }

    pub fn transfer(&mut self, from: ActorId, to: ActorId, amount: u128) {
        let bal = self.balances.entry(from).or_default();
        assert!(*bal >= amount, "Insufficient balance");
        *bal -= amount;
        *self.balances.entry(to).or_default() += amount;
        let _ = self.emit_event(VftEvent::Transferred(Transfer { from, to, amount }));
    }

    pub fn balance_of(&self, who: ActorId) -> u128 {
        *self.balances.get(&who).unwrap_or(&0)
    }
}
