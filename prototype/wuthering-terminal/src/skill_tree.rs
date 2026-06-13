use crate::components::{CharacterClass, SkillSlot, SkillTree, SkillUpgrade, UpgradeEffect};

impl SkillTree {
    pub fn for_class(class: CharacterClass) -> Self {
        match class {
            CharacterClass::Warrior => warrior_skill_tree(),
            CharacterClass::Mage => mage_skill_tree(),
            CharacterClass::Healer => healer_skill_tree(),
            CharacterClass::Rogue => rogue_skill_tree(),
            CharacterClass::DestructibleObject => Self {
                skill_points: 0,
                upgrades: Vec::new(),
            },
            _ => Self {
                skill_points: 0,
                upgrades: Vec::new(),
            },
        }
    }

    pub fn available_upgrades(&self, slot: SkillSlot) -> Vec<&SkillUpgrade> {
        self.upgrades
            .iter()
            .filter(|u| u.skill_slot == slot && !u.unlocked)
            .filter(|u| {
                u.prerequisites.is_empty()
                    || u.prerequisites.iter().all(|pre| {
                        self.upgrades
                            .iter()
                            .any(|p| p.upgrade_id == *pre && p.unlocked)
                    })
            })
            .collect()
    }

    pub fn can_unlock(&self, upgrade_id: &str) -> bool {
        if self.skill_points == 0 {
            return false;
        }
        let upgrade = match self.upgrades.iter().find(|u| u.upgrade_id == upgrade_id) {
            Some(u) => u,
            None => return false,
        };
        if upgrade.unlocked {
            return false;
        }
        if self.skill_points < upgrade.cost {
            return false;
        }
        upgrade.prerequisites.iter().all(|pre| {
            self.upgrades
                .iter()
                .any(|p| p.upgrade_id == *pre && p.unlocked)
        })
    }

    pub fn unlock(&mut self, upgrade_id: &str) -> bool {
        if !self.can_unlock(upgrade_id) {
            return false;
        }
        if let Some(upgrade) = self
            .upgrades
            .iter_mut()
            .find(|u| u.upgrade_id == upgrade_id)
        {
            self.skill_points -= upgrade.cost;
            upgrade.unlocked = true;
            true
        } else {
            false
        }
    }

    pub fn total_damage_bonus(&self, slot: SkillSlot) -> i32 {
        self.upgrades
            .iter()
            .filter(|u| u.skill_slot == slot && u.unlocked)
            .map(|u| match &u.effect {
                UpgradeEffect::DamageBoost(v) => *v,
                _ => 0,
            })
            .sum()
    }

    pub fn total_range_bonus(&self, slot: SkillSlot) -> i32 {
        self.upgrades
            .iter()
            .filter(|u| u.skill_slot == slot && u.unlocked)
            .map(|u| match &u.effect {
                UpgradeEffect::RangeBoost(v) => *v,
                _ => 0,
            })
            .sum()
    }

    pub fn total_aoe_bonus(&self, slot: SkillSlot) -> i32 {
        self.upgrades
            .iter()
            .filter(|u| u.skill_slot == slot && u.unlocked)
            .map(|u| match &u.effect {
                UpgradeEffect::AoEBonus(v) => *v,
                _ => 0,
            })
            .sum()
    }

    pub fn total_heal_bonus(&self) -> i32 {
        self.upgrades
            .iter()
            .filter(|u| u.unlocked)
            .map(|u| match &u.effect {
                UpgradeEffect::HealBoost(v) => *v,
                _ => 0,
            })
            .sum()
    }

    pub fn total_passive_stats(&self) -> (i32, i32, i32, i32) {
        let mut atk = 0;
        let mut def = 0;
        let mut hp = 0;
        let mut spd = 0;
        for u in &self.upgrades {
            if u.unlocked {
                if let UpgradeEffect::Passive {
                    atk: a,
                    def: d,
                    hp: h,
                    spd: s,
                } = &u.effect
                {
                    atk += a;
                    def += d;
                    hp += h;
                    spd += s;
                }
            }
        }
        (atk, def, hp, spd)
    }

    pub fn unlocked_count(&self) -> usize {
        self.upgrades.iter().filter(|u| u.unlocked).count()
    }
}

fn warrior_skill_tree() -> SkillTree {
    SkillTree {
        skill_points: 0,
        upgrades: vec![
            // Skill 1 (Slash) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "warrior_s1_power".into(),
                name: "Power Strike".into(),
                description: "Slash deals +5 damage.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(5),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "warrior_s1_cleave".into(),
                name: "Cleave".into(),
                description: "Slash hits adjacent enemies.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::AoEBonus(1),
                prerequisites: vec!["warrior_s1_power".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "warrior_s1_executioner".into(),
                name: "Executioner".into(),
                description: "+50% damage vs targets below 30% HP.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(15),
                prerequisites: vec!["warrior_s1_cleave".into()],
            },
            // Skill 2 (Shield Bash) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "warrior_s2_stun".into(),
                name: "Stunning Blow".into(),
                description: "Shield Bash gains +20% stun chance.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::StatusChance(20),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "warrior_s2_concussion".into(),
                name: "Concussion".into(),
                description: "Stun lasts +1 turn.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(3),
                prerequisites: vec!["warrior_s2_stun".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "warrior_s2_earthquake".into(),
                name: "Earthquake".into(),
                description: "Shield Bash becomes AoE stun.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::AoEBonus(1),
                prerequisites: vec!["warrior_s2_concussion".into()],
            },
            // Skill 3 (Charge) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "warrior_s3_range".into(),
                name: "Extended Charge".into(),
                description: "Charge range +1.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::RangeBoost(1),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "warrior_s3_impact".into(),
                name: "Impact".into(),
                description: "Charge deals +10 damage.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(10),
                prerequisites: vec!["warrior_s3_range".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "warrior_s3_juggernaut".into(),
                name: "Juggernaut".into(),
                description: "Charge pierces through enemies.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::PierceResist(10),
                prerequisites: vec!["warrior_s3_impact".into()],
            },
            // Passive tree
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "warrior_passive_iron_will".into(),
                name: "Iron Will".into(),
                description: "+5 DEF.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 5,
                    hp: 0,
                    spd: 0,
                },
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "warrior_passive_unbreakable".into(),
                name: "Unbreakable".into(),
                description: "+20 max HP.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 0,
                    hp: 20,
                    spd: 0,
                },
                prerequisites: vec!["warrior_passive_iron_will".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "warrior_passive_berserker".into(),
                name: "Berserker".into(),
                description: "Gain ATK when below 30% HP.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 10,
                    def: 0,
                    hp: 0,
                    spd: 2,
                },
                prerequisites: vec!["warrior_passive_unbreakable".into()],
            },
        ],
    }
}

fn mage_skill_tree() -> SkillTree {
    SkillTree {
        skill_points: 0,
        upgrades: vec![
            // Skill 1 (Arcane Bolt) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "mage_s1_power".into(),
                name: "Arcane Power".into(),
                description: "Arcane Bolt deals +8 damage.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(8),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "mage_s1_chain".into(),
                name: "Chain Lightning".into(),
                description: "Arcane Bolt chains to 2 additional targets.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::ExtraHit(2),
                prerequisites: vec!["mage_s1_power".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "mage_s1_overload".into(),
                name: "Overload".into(),
                description: "Chains to 3 targets, applies Lightning shatter.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::ExtraHit(1),
                prerequisites: vec!["mage_s1_chain".into()],
            },
            // Skill 2 (Ice Shard) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "mage_s2_deep_freeze".into(),
                name: "Deep Freeze".into(),
                description: "Ice duration +2 turns.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::StatusChance(25),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "mage_s2_blizzard".into(),
                name: "Blizzard".into(),
                description: "Ice Shard becomes AoE.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::AoEBonus(1),
                prerequisites: vec!["mage_s2_deep_freeze".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "mage_s2_permafrost".into(),
                name: "Permafrost".into(),
                description: "Creates a permanent ice zone.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(12),
                prerequisites: vec!["mage_s2_blizzard".into()],
            },
            // Skill 3 (Lightning Storm) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "mage_s3_wider".into(),
                name: "Wider Range".into(),
                description: "Lightning Storm radius +1.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::RangeBoost(1),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "mage_s3_thunderstruck".into(),
                name: "Thunderstruck".into(),
                description: "Gains stun chance on hit.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::StatusChance(15),
                prerequisites: vec!["mage_s3_wider".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "mage_s3_cataclysm".into(),
                name: "Cataclysm".into(),
                description: "Massive AoE lightning devastation.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::AoEBonus(2),
                prerequisites: vec!["mage_s3_thunderstruck".into()],
            },
            // Passive tree
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "mage_passive_mana_flow".into(),
                name: "Mana Flow".into(),
                description: "+1 max AP.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 0,
                    hp: 0,
                    spd: 0,
                },
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "mage_passive_arcane_mastery".into(),
                name: "Arcane Mastery".into(),
                description: "+15 ATK.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 15,
                    def: 0,
                    hp: 0,
                    spd: 0,
                },
                prerequisites: vec!["mage_passive_mana_flow".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "mage_passive_elemental_affinity".into(),
                name: "Elemental Affinity".into(),
                description: "Double elemental damage.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 10,
                    def: 0,
                    hp: 0,
                    spd: 3,
                },
                prerequisites: vec!["mage_passive_arcane_mastery".into()],
            },
        ],
    }
}

fn rogue_skill_tree() -> SkillTree {
    SkillTree {
        skill_points: 0,
        upgrades: vec![
            // Skill 1 (Attack) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "rogue_s1_venom_edge".into(),
                name: "Venom Edge".into(),
                description: "Basic attacks deal +3 damage.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(3),
                prerequisites: vec![],
            },
            // Skill 2 (Shadow Step / Poison Strike) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "rogue_s2_toxic_burst".into(),
                name: "Toxic Burst".into(),
                description: "Skill 2 deals +10 damage.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(10),
                prerequisites: vec![],
            },
            // Skill 3 (Flurry) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "rogue_s3_flurry".into(),
                name: "Blade Flurry".into(),
                description: "Skill 3 damage increased by 15.".into(),
                tier: 1,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(15),
                prerequisites: vec![],
            },
            // Passive
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "rogue_p_evasion".into(),
                name: "Evasion".into(),
                description: "Increase Speed by +2 and Defense by +2.".into(),
                tier: 1,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 2,
                    hp: 0,
                    spd: 2,
                },
                prerequisites: vec![],
            },
        ],
    }
}

fn healer_skill_tree() -> SkillTree {
    SkillTree {
        skill_points: 0,
        upgrades: vec![
            // Skill 1 (Heal) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "healer_s1_greater_heal".into(),
                name: "Greater Heal".into(),
                description: "Heal restores +15 HP.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::HealBoost(15),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "healer_s1_mass_heal".into(),
                name: "Mass Heal".into(),
                description: "Heal affects 2 additional allies.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::HealBoost(10),
                prerequisites: vec!["healer_s1_greater_heal".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill1,
                upgrade_id: "healer_s1_divine_light".into(),
                name: "Divine Light".into(),
                description: "Full heal + cleanse all debuffs.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::HealBoost(50),
                prerequisites: vec!["healer_s1_mass_heal".into()],
            },
            // Skill 2 (Cleanse) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "healer_s2_purify".into(),
                name: "Purify".into(),
                description: "Removes all debuffs from target.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::StatusChance(100),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "healer_s2_ward".into(),
                name: "Ward".into(),
                description: "Cleanse grants a shield.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(5),
                prerequisites: vec!["healer_s2_purify".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill2,
                upgrade_id: "healer_s2_sanctuary".into(),
                name: "Sanctuary".into(),
                description: "AoE cleanse and heal zone.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::AoEBonus(1),
                prerequisites: vec!["healer_s2_ward".into()],
            },
            // Skill 3 (Holy Aura) tree
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "healer_s3_extended".into(),
                name: "Extended Aura".into(),
                description: "Holy Aura duration +2 turns.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::CooldownReduction(2),
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "healer_s3_radiance".into(),
                name: "Radiance".into(),
                description: "Holy Aura damages nearby enemies.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::DamageBoost(8),
                prerequisites: vec!["healer_s3_extended".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Skill3,
                upgrade_id: "healer_s3_salvation".into(),
                name: "Salvation".into(),
                description: "Revive one fallen ally per battle.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::HealBoost(30),
                prerequisites: vec!["healer_s3_radiance".into()],
            },
            // Passive tree
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "healer_passive_gentle_touch".into(),
                name: "Gentle Touch".into(),
                description: "+5 heal bonus on all heals.".into(),
                tier: 1,
                cost: 1,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 0,
                    hp: 5,
                    spd: 0,
                },
                prerequisites: vec![],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "healer_passive_resilient".into(),
                name: "Resilient".into(),
                description: "+10 DEF.".into(),
                tier: 2,
                cost: 2,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 10,
                    hp: 0,
                    spd: 0,
                },
                prerequisites: vec!["healer_passive_gentle_touch".into()],
            },
            SkillUpgrade {
                skill_slot: SkillSlot::Passive,
                upgrade_id: "healer_passive_guardian_angel".into(),
                name: "Guardian Angel".into(),
                description: "Auto-revive once per battle.".into(),
                tier: 3,
                cost: 3,
                unlocked: false,
                effect: UpgradeEffect::Passive {
                    atk: 0,
                    def: 5,
                    hp: 30,
                    spd: 0,
                },
                prerequisites: vec!["healer_passive_resilient".into()],
            },
        ],
    }
}
