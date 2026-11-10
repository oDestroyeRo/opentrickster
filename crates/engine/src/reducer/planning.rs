use super::*;

impl Engine {
    pub(super) fn safety_blockers(&self) -> Vec<Blocker> {
        let Some(setup) = &self.setup else {
            return vec![Blocker::NeedsSetup];
        };
        let draft = setup.draft();
        let digging = matches!(draft.activity, Activity::Digging { .. });
        let mut blockers = self.book.missing_or_stale(self.now, digging);
        if let Some(support) = &self.support {
            if !support.quiesce {
                blockers.push(Blocker::NeedsCapability(Capability::Quiescence));
            }
            if digging && !support.dig_cycle {
                blockers.push(Blocker::NeedsCapability(Capability::DigCycle));
            }
            if !digging && !support.basic_attack {
                blockers.push(Blocker::NeedsCapability(Capability::BasicAttack));
            }
            // Combat gains only come from pickups, so a combat goal could never be reached.
            if !digging && draft.goal.is_some() && !support.pickup {
                blockers.push(Blocker::NeedsCapability(Capability::Pickup));
            }
        }
        if let Some((map, point)) = self.book.own.position {
            if map != draft.boundary.parts().0 {
                blockers.push(Blocker::NeedsBoundaryMap(draft.boundary.parts().0));
            } else if !draft.boundary.contains(map, point) {
                blockers.push(Blocker::NeedsInsideBoundary);
            }
        }
        if let Some(vitals) = &self.book.own.vitals {
            if !digging {
                match vitals.hp_potions {
                    None => blockers.push(Blocker::NeedsObservation(StateKind::HpPotions)),
                    Some(0) => blockers.push(Blocker::NeedsHpPotions),
                    Some(_) => {}
                }
            }

            if let Some(hp) = &vitals.hp {
                if hp.current == 0 {
                    blockers.push(Blocker::NeedsLivingCharacter);
                }
                match hp.maximum {
                    None => blockers.push(Blocker::NeedsObservation(StateKind::HpMaximum)),
                    Some(max)
                        if u64::from(hp.current) * 100
                            <= u64::from(max.get()) * u64::from(draft.hp_at.value()) =>
                    {
                        if !self
                            .support
                            .as_ref()
                            .is_some_and(|support| support.hp_potion)
                        {
                            blockers.push(Blocker::NeedsCapability(Capability::HpPotion));
                        }
                        if self.book.hp_potion().is_none() {
                            blockers.push(Blocker::NeedsObservation(StateKind::HpPotionStack));
                        } else if self
                            .book
                            .hp_potion()
                            .and_then(|key| self.book.inventory.stacks.get(&key))
                            .is_none_or(|stack| stack.quantity == 0)
                        {
                            blockers.push(Blocker::NeedsHpPotions);
                        }
                        if blockers.iter().any(|blocker| {
                            matches!(
                                blocker,
                                Blocker::NeedsCapability(Capability::HpPotion)
                                    | Blocker::NeedsObservation(StateKind::HpPotionStack)
                                    | Blocker::NeedsHpPotions
                            )
                        }) {
                            blockers.push(Blocker::NeedsHpAbove {
                                percent: draft.hp_at.value(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            if let Some(weight) = &vitals.weight {
                match weight.maximum {
                    None => blockers.push(Blocker::NeedsObservation(StateKind::WeightMaximum)),
                    Some(max)
                        if u64::from(weight.current) * 100
                            >= u64::from(max.get()) * u64::from(draft.weight_limit.value()) =>
                    {
                        blockers.push(Blocker::NeedsWeightBelow {
                            percent: draft.weight_limit.value(),
                        })
                    }
                    _ => {}
                }
            }
        }
        if let Activity::Digging { min_drill_life } = draft.activity {
            match self.book.own.drill {
                Drill::Unknown => {
                    if !blockers.contains(&Blocker::NeedsObservation(StateKind::Drill)) {
                        blockers.push(Blocker::NeedsObservation(StateKind::Drill));
                    }
                }
                Drill::Absent => blockers.push(Blocker::NeedsEquippedDrill),
                Drill::Equipped { life, depth } => {
                    if life < min_drill_life {
                        blockers.push(Blocker::NeedsDrillLife {
                            minimum: min_drill_life,
                        });
                    }
                    if depth == 0 {
                        blockers.push(Blocker::NeedsDrillDepth);
                    }
                }
            }
        }
        if let Some(run) = &self.run
            && let Some(goal) = &draft.goal
            && run.totals.goal_gained >= u64::from(goal.gained)
        {
            blockers.push(Blocker::GoalReached {
                gained: run.totals.goal_gained,
                target: goal.gained,
            });
        }
        blockers
    }

    pub(super) fn plan(&self) -> Result<Option<Action>, Blocker> {
        let setup = self.setup.as_ref().ok_or(Blocker::NeedsSetup)?.draft();
        let (map, own) = self
            .book
            .own
            .position
            .ok_or(Blocker::NeedsObservation(StateKind::Position))?;
        let support = self.support.as_ref().ok_or(Blocker::NeedsConnection)?;
        if let Some(hp) = self
            .book
            .own
            .vitals
            .as_ref()
            .and_then(|vitals| vitals.hp.as_ref())
            && let Some(maximum) = hp.maximum
            && u64::from(hp.current) * 100
                <= u64::from(maximum.get()) * u64::from(setup.hp_at.value())
        {
            if !support.hp_potion {
                return Err(Blocker::NeedsCapability(Capability::HpPotion));
            }
            let key = self
                .book
                .hp_potion()
                .ok_or(Blocker::NeedsObservation(StateKind::HpPotionStack))?;
            if self
                .book
                .inventory
                .stacks
                .get(&key)
                .is_none_or(|stack| stack.quantity == 0)
            {
                return Err(Blocker::NeedsHpPotions);
            }
            return Ok(Some(Action::HpPotion(key)));
        }
        let move_or = |point, action| {
            if self.book.reached(point) {
                Ok(Some(action))
            } else if support.move_to {
                Ok(Some(Action::MoveTo(point)))
            } else {
                Err(Blocker::NeedsCapability(Capability::Movement))
            }
        };
        // Without an adapter pickup capability, drops are left on the ground; they never block a run.
        if support.pickup
            && let Some(drop) = self
                .book
                .drops
                .values()
                .filter(|drop| {
                    drop.quantity > 0
                        && setup.boundary.contains(map, drop.position)
                        && self.book.drop_fresh(drop.key, self.now)
                })
                .min_by_key(|drop| (distance(own, drop.position), drop.key))
        {
            return move_or(drop.position, Action::Pickup(drop.key));
        }
        match &setup.activity {
            Activity::Combat { targets } => {
                let target = self
                    .book
                    .entities
                    .values()
                    .filter(|entity| {
                        entity.alive
                            && entity.hp.as_ref().is_none_or(|hp| hp.current > 0)
                            && entity.engaged_by_other == Some(false)
                            && self.book.entity_fresh(entity.key, self.now)
                            && self.run.as_ref().is_none_or(|run| {
                                run.set_aside
                                    .get(&entity.key)
                                    .is_none_or(|until| self.now >= *until)
                            })
                            && setup.boundary.contains(map, entity.position)
                            && match targets {
                                Targets::Only(keys) => keys.contains(&entity.species),
                                Targets::Except(keys) => !keys.contains(&entity.species),
                            }
                    })
                    .min_by_key(|entity| (distance(own, entity.position), entity.key));
                // A normal attack click approaches the target; leaving the boundary pauses the run.
                Ok(target.map(|entity| Action::BasicAttack(entity.key)))
            }
            Activity::Digging { .. } => {
                let (_, center, radius) = setup.boundary.parts();
                // Five fixed points avoid allocating a grid or inventing a spacing formula.
                // MoveTo is a bounded destination request; the adapter must prove normal pathfinding.
                let r = i32::from(radius.get());
                let points = [
                    center,
                    Point {
                        x: center.x + r,
                        y: center.y,
                    },
                    Point {
                        x: center.x,
                        y: center.y + r,
                    },
                    Point {
                        x: center.x - r,
                        y: center.y,
                    },
                    Point {
                        x: center.x,
                        y: center.y - r,
                    },
                ];
                let index = self.run.as_ref().map_or(0, |run| run.dig_index % 5) as usize;
                // The server can end a walk short of an unsnapped or unwalkable point; dig there.
                move_or(points[index], Action::DigAt(own))
            }
        }
    }
}
