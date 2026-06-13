use crate::components::{
    CharacterClass, Element, Equipment, EquipmentSet, EquipmentSlot, EquipmentSpecial,
    SetBonusStats,
};

const UPGRADE_MULTIPLIER: f32 = 0.25;

pub fn upgrade_stat(base: i32, level: u8) -> i32 {
    let bonus = (base as f32 * UPGRADE_MULTIPLIER * level as f32).round() as i32;
    base + bonus
}

pub fn upgrade_equipment(item: &mut Equipment) -> bool {
    if item.upgrade_level >= 3 {
        return false;
    }
    item.upgrade_level += 1;
    item.atk_bonus = upgrade_stat(item.base_atk, item.upgrade_level);
    item.def_bonus = upgrade_stat(item.base_def, item.upgrade_level);
    item.hp_bonus = upgrade_stat(item.base_hp, item.upgrade_level);
    item.spd_bonus = upgrade_stat(item.base_spd, item.upgrade_level);
    true
}

pub fn check_set_bonuses(equipped: &crate::components::EquippedItems) -> SetBonusStats {
    let mut result = SetBonusStats::default();

    let all_names: Vec<&str> = [
        equipped.weapon.as_ref().map(|e| e.name.as_str()),
        equipped.armor.as_ref().map(|e| e.name.as_str()),
        equipped.accessory.as_ref().map(|e| e.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect();

    for set in &[
        EquipmentSet::ShadowKnight,
        EquipmentSet::ArcaneWeaver,
        EquipmentSet::DivineGuardian,
    ] {
        let required = set.required_items();
        let has_all = required.iter().all(|name| all_names.contains(name));
        if has_all {
            result.atk_multiplier *= set.atk_multiplier();
            result.def_multiplier *= set.def_multiplier();
            result.hp_multiplier *= set.hp_multiplier();
            result.spd_multiplier *= set.spd_multiplier();
            result.active_sets.push(*set);
        }
    }

    result
}

macro_rules! equip {
    ($name:expr, $slot:expr, atk=$atk:expr, def=$def:expr, hp=$hp:expr, spd=$spd:expr, special=$special:expr, set=$set:expr) => {
        Equipment {
            name: $name.into(),
            slot: $slot,
            atk_bonus: $atk,
            def_bonus: $def,
            hp_bonus: $hp,
            spd_bonus: $spd,
            special: $special,
            upgrade_level: 0,
            set_id: $set,
            base_atk: $atk,
            base_def: $def,
            base_hp: $hp,
            base_spd: $spd,
        }
    };
}

pub fn shadow_dagger() -> Equipment {
    equip!(
        "Shadow Dagger",
        EquipmentSlot::Weapon,
        atk = 6,
        def = 0,
        hp = 0,
        spd = 2,
        special = Some(EquipmentSpecial::CritBoost(15)),
        set = None
    )
}

pub fn leather_armor() -> Equipment {
    equip!(
        "Leather Armor",
        EquipmentSlot::Armor,
        atk = 0,
        def = 3,
        hp = 5,
        spd = 1,
        special = None,
        set = None
    )
}

pub fn iron_sword() -> Equipment {
    equip!(
        "Iron Sword",
        EquipmentSlot::Weapon,
        atk = 5,
        def = 0,
        hp = 0,
        spd = 0,
        special = None,
        set = None
    )
}

pub fn flame_blade() -> Equipment {
    equip!(
        "Flame Blade",
        EquipmentSlot::Weapon,
        atk = 8,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::ElementalDamage(Element::Physical, 5)),
        set = None
    )
}

pub fn staff_of_storms() -> Equipment {
    equip!(
        "Staff of Storms",
        EquipmentSlot::Weapon,
        atk = 10,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::ElementalDamage(Element::Lightning, 8)),
        set = None
    )
}

pub fn healing_wand() -> Equipment {
    equip!(
        "Healing Wand",
        EquipmentSlot::Weapon,
        atk = 3,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::HpRegen(5)),
        set = None
    )
}

pub fn chain_mail() -> Equipment {
    equip!(
        "Chain Mail",
        EquipmentSlot::Armor,
        atk = 0,
        def = 5,
        hp = 0,
        spd = 0,
        special = None,
        set = None
    )
}

pub fn robe_of_warding() -> Equipment {
    equip!(
        "Robe of Warding",
        EquipmentSlot::Armor,
        atk = 0,
        def = 3,
        hp = 0,
        spd = 2,
        special = None,
        set = None
    )
}

pub fn holy_vestments() -> Equipment {
    equip!(
        "Holy Vestments",
        EquipmentSlot::Armor,
        atk = 0,
        def = 4,
        hp = 20,
        spd = 0,
        special = None,
        set = None
    )
}

pub fn plate_armor() -> Equipment {
    equip!(
        "Plate Armor",
        EquipmentSlot::Armor,
        atk = 0,
        def = 8,
        hp = 0,
        spd = -1,
        special = None,
        set = None
    )
}

pub fn speed_ring() -> Equipment {
    equip!(
        "Speed Ring",
        EquipmentSlot::Accessory,
        atk = 0,
        def = 0,
        hp = 0,
        spd = 3,
        special = None,
        set = None
    )
}

pub fn life_amulet() -> Equipment {
    equip!(
        "Life Amulet",
        EquipmentSlot::Accessory,
        atk = 0,
        def = 0,
        hp = 30,
        spd = 0,
        special = None,
        set = None
    )
}

pub fn crit_gem() -> Equipment {
    equip!(
        "Crit Gem",
        EquipmentSlot::Accessory,
        atk = 0,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::CritBoost(15)),
        set = None
    )
}

pub fn vampiric_ring() -> Equipment {
    equip!(
        "Vampiric Ring",
        EquipmentSlot::Accessory,
        atk = 0,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::LifestealPercent(10)),
        set = None
    )
}

pub fn equipment_for_class(class: CharacterClass) -> Vec<Equipment> {
    match class {
        CharacterClass::Warrior => vec![iron_sword(), chain_mail()],
        CharacterClass::Rogue => vec![shadow_dagger(), leather_armor()],
        CharacterClass::Mage => vec![staff_of_storms(), robe_of_warding()],
        CharacterClass::Healer => vec![healing_wand(), holy_vestments()],
        CharacterClass::DestructibleObject => vec![],
        _ => vec![],
    }
}

pub fn set_reward_for_defeated_class(class: CharacterClass) -> Option<(CharacterClass, Equipment)> {
    match class {
        CharacterClass::ShadowStalker => Some((CharacterClass::Warrior, shadow_armor())),
        CharacterClass::CursedSentinel => Some((CharacterClass::Warrior, dark_blade())),
        CharacterClass::PlagueWraith => Some((CharacterClass::Mage, arcane_robe())),
        CharacterClass::GlacialGolem => Some((CharacterClass::Mage, arcane_staff())),
        CharacterClass::EnemyCleric => Some((CharacterClass::Healer, divine_staff())),
        CharacterClass::Boss => Some((CharacterClass::Healer, divine_vestments())),
        CharacterClass::DestructibleObject => None,
        _ => None,
    }
}

pub fn dark_blade() -> Equipment {
    equip!(
        "DarkBlade",
        EquipmentSlot::Weapon,
        atk = 12,
        def = 0,
        hp = 0,
        spd = 1,
        special = Some(EquipmentSpecial::CritBoost(10)),
        set = Some(EquipmentSet::ShadowKnight)
    )
}

pub fn shadow_armor() -> Equipment {
    equip!(
        "ShadowArmor",
        EquipmentSlot::Armor,
        atk = 2,
        def = 6,
        hp = 10,
        spd = 2,
        special = None,
        set = Some(EquipmentSet::ShadowKnight)
    )
}

pub fn arcane_staff() -> Equipment {
    equip!(
        "ArcaneStaff",
        EquipmentSlot::Weapon,
        atk = 14,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::ElementalDamage(Element::Lightning, 12)),
        set = Some(EquipmentSet::ArcaneWeaver)
    )
}

pub fn arcane_robe() -> Equipment {
    equip!(
        "ArcaneRobe",
        EquipmentSlot::Armor,
        atk = 3,
        def = 4,
        hp = 0,
        spd = 3,
        special = None,
        set = Some(EquipmentSet::ArcaneWeaver)
    )
}

pub fn divine_staff() -> Equipment {
    equip!(
        "DivineStaff",
        EquipmentSlot::Weapon,
        atk = 6,
        def = 0,
        hp = 0,
        spd = 0,
        special = Some(EquipmentSpecial::HpRegen(10)),
        set = Some(EquipmentSet::DivineGuardian)
    )
}

pub fn divine_vestments() -> Equipment {
    equip!(
        "DivineVestments",
        EquipmentSlot::Armor,
        atk = 0,
        def = 6,
        hp = 40,
        spd = 0,
        special = None,
        set = Some(EquipmentSet::DivineGuardian)
    )
}

pub fn upgrade_kit() -> crate::components::Item {
    crate::components::Item {
        name: "Upgrade Kit".into(),
        effect: crate::components::ItemEffect::UpgradeKit,
        consumed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::EquippedItems;

    #[test]
    fn test_equip_and_unequip() {
        let mut eq = EquippedItems::default();
        let sword = iron_sword();
        let old = eq.equip(sword);
        assert!(old.is_none());
        assert_eq!(eq.weapon.as_ref().unwrap().name, "Iron Sword");

        let chain = chain_mail();
        let old = eq.equip(chain);
        assert!(old.is_none());
        assert_eq!(eq.armor.as_ref().unwrap().name, "Chain Mail");

        let removed = eq.unequip(EquipmentSlot::Weapon);
        assert_eq!(removed.unwrap().name, "Iron Sword");
        assert!(eq.weapon.is_none());
    }

    #[test]
    fn test_slot_replacement() {
        let mut eq = EquippedItems::default();
        eq.equip(iron_sword());
        let replaced = eq.equip(flame_blade());
        assert_eq!(replaced.unwrap().name, "Iron Sword");
        assert_eq!(eq.weapon.as_ref().unwrap().name, "Flame Blade");
    }

    #[test]
    fn test_stat_bonus_aggregation() {
        let mut eq = EquippedItems::default();
        eq.equip(iron_sword());
        eq.equip(chain_mail());
        eq.equip(speed_ring());

        assert_eq!(eq.total_atk_bonus(), 5);
        assert_eq!(eq.total_def_bonus(), 5);
        assert_eq!(eq.total_hp_bonus(), 0);
        assert_eq!(eq.total_spd_bonus(), 3);
    }

    #[test]
    fn test_equipment_for_class_warrior() {
        let gear = equipment_for_class(CharacterClass::Warrior);
        assert_eq!(gear.len(), 2);
        assert_eq!(gear[0].slot, EquipmentSlot::Weapon);
        assert_eq!(gear[1].slot, EquipmentSlot::Armor);
    }

    #[test]
    fn test_equipment_for_class_mage() {
        let gear = equipment_for_class(CharacterClass::Mage);
        assert_eq!(gear.len(), 2);
        assert!(gear.iter().any(|e| e.name == "Staff of Storms"));
        assert!(gear.iter().any(|e| e.name == "Robe of Warding"));
    }

    #[test]
    fn test_equipment_for_class_healer() {
        let gear = equipment_for_class(CharacterClass::Healer);
        assert_eq!(gear.len(), 2);
        assert!(gear.iter().any(|e| e.name == "Healing Wand"));
        assert!(gear.iter().any(|e| e.name == "Holy Vestments"));
    }

    #[test]
    fn test_equipment_for_enemy_class_returns_empty() {
        let gear = equipment_for_class(CharacterClass::Boss);
        assert!(gear.is_empty());
        let gear = equipment_for_class(CharacterClass::ShadowStalker);
        assert!(gear.is_empty());
    }

    #[test]
    fn test_set_reward_for_defeated_class() {
        let (hero, equipment) = set_reward_for_defeated_class(CharacterClass::CursedSentinel)
            .expect("sentinel should award a set piece");
        assert_eq!(hero, CharacterClass::Warrior);
        assert_eq!(equipment.name, "DarkBlade");
        assert_eq!(equipment.set_id, Some(EquipmentSet::ShadowKnight));

        let (hero, equipment) = set_reward_for_defeated_class(CharacterClass::EnemyCleric)
            .expect("cleric should award a set piece");
        assert_eq!(hero, CharacterClass::Healer);
        assert_eq!(equipment.name, "DivineStaff");

        assert!(set_reward_for_defeated_class(CharacterClass::CorruptedSpore).is_none());
    }

    #[test]
    fn test_special_effects_exist() {
        assert!(shadow_dagger().special.is_some());
        assert!(vampiric_ring().special.is_some());
        assert!(iron_sword().special.is_none());
    }

    #[test]
    fn test_unequip_empty_slot_returns_none() {
        let mut eq = EquippedItems::default();
        assert!(eq.unequip(EquipmentSlot::Weapon).is_none());
        assert!(eq.unequip(EquipmentSlot::Armor).is_none());
        assert!(eq.unequip(EquipmentSlot::Accessory).is_none());
    }

    #[test]
    fn test_full_equip_all_slots() {
        let mut eq = EquippedItems::default();
        eq.equip(iron_sword());
        eq.equip(chain_mail());
        eq.equip(life_amulet());

        assert_eq!(eq.total_atk_bonus(), 5);
        assert_eq!(eq.total_def_bonus(), 5);
        assert_eq!(eq.total_hp_bonus(), 30);
        assert_eq!(eq.total_spd_bonus(), 0);
    }

    #[test]
    fn test_plate_armor_negative_spd() {
        let armor = plate_armor();
        assert_eq!(armor.spd_bonus, -1);
        assert_eq!(armor.def_bonus, 8);
    }

    #[test]
    fn test_set_items_have_set_id() {
        assert_eq!(dark_blade().set_id, Some(EquipmentSet::ShadowKnight));
        assert_eq!(shadow_armor().set_id, Some(EquipmentSet::ShadowKnight));
        assert_eq!(arcane_staff().set_id, Some(EquipmentSet::ArcaneWeaver));
        assert_eq!(arcane_robe().set_id, Some(EquipmentSet::ArcaneWeaver));
        assert_eq!(divine_staff().set_id, Some(EquipmentSet::DivineGuardian));
        assert_eq!(
            divine_vestments().set_id,
            Some(EquipmentSet::DivineGuardian)
        );
    }

    #[test]
    fn test_regular_items_have_no_set_id() {
        assert!(iron_sword().set_id.is_none());
        assert!(chain_mail().set_id.is_none());
        assert!(speed_ring().set_id.is_none());
    }

    #[test]
    fn test_check_set_bonuses_shadow_knight() {
        let mut eq = EquippedItems::default();
        eq.equip(dark_blade());
        eq.equip(shadow_armor());
        let bonuses = check_set_bonuses(&eq);
        assert_eq!(bonuses.active_sets.len(), 1);
        assert_eq!(bonuses.active_sets[0], EquipmentSet::ShadowKnight);
        assert!((bonuses.atk_multiplier - 1.15).abs() < 0.01);
        assert!((bonuses.def_multiplier - 1.05).abs() < 0.01);
    }

    #[test]
    fn test_check_set_bonuses_arcane_weaver() {
        let mut eq = EquippedItems::default();
        eq.equip(arcane_staff());
        eq.equip(arcane_robe());
        let bonuses = check_set_bonuses(&eq);
        assert_eq!(bonuses.active_sets.len(), 1);
        assert_eq!(bonuses.active_sets[0], EquipmentSet::ArcaneWeaver);
        assert!((bonuses.atk_multiplier - 1.10).abs() < 0.01);
        assert!((bonuses.spd_multiplier - 1.10).abs() < 0.01);
    }

    #[test]
    fn test_check_set_bonuses_divine_guardian() {
        let mut eq = EquippedItems::default();
        eq.equip(divine_staff());
        eq.equip(divine_vestments());
        let bonuses = check_set_bonuses(&eq);
        assert_eq!(bonuses.active_sets.len(), 1);
        assert_eq!(bonuses.active_sets[0], EquipmentSet::DivineGuardian);
        assert!((bonuses.def_multiplier - 1.15).abs() < 0.01);
        assert!((bonuses.hp_multiplier - 1.10).abs() < 0.01);
    }

    #[test]
    fn test_check_set_bonuses_no_set() {
        let mut eq = EquippedItems::default();
        eq.equip(iron_sword());
        eq.equip(chain_mail());
        eq.equip(speed_ring());
        let bonuses = check_set_bonuses(&eq);
        assert!(bonuses.active_sets.is_empty());
        assert!((bonuses.atk_multiplier - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_check_set_bonuses_partial_set() {
        let mut eq = EquippedItems::default();
        eq.equip(dark_blade());
        let bonuses = check_set_bonuses(&eq);
        assert!(bonuses.active_sets.is_empty());
    }

    #[test]
    fn test_upgrade_equipment_basic() {
        let mut sword = iron_sword();
        assert_eq!(sword.upgrade_level, 0);
        assert!(upgrade_equipment(&mut sword));
        assert_eq!(sword.upgrade_level, 1);
        assert_eq!(sword.atk_bonus, 6);
    }

    #[test]
    fn test_upgrade_equipment_to_max() {
        let mut sword = iron_sword();
        assert!(upgrade_equipment(&mut sword));
        assert!(upgrade_equipment(&mut sword));
        assert!(upgrade_equipment(&mut sword));
        assert_eq!(sword.upgrade_level, 3);
        assert!(!upgrade_equipment(&mut sword));
        assert_eq!(sword.upgrade_level, 3);
    }

    #[test]
    fn test_upgrade_stat_calculation() {
        assert_eq!(upgrade_stat(10, 1), 13);
        assert_eq!(upgrade_stat(10, 2), 15);
        assert_eq!(upgrade_stat(10, 3), 18);
    }

    #[test]
    fn test_upgrade_defensive_item() {
        let mut armor = chain_mail();
        upgrade_equipment(&mut armor);
        assert_eq!(armor.upgrade_level, 1);
        assert_eq!(armor.def_bonus, 6);
    }

    #[test]
    fn test_upgrade_negative_stat() {
        let mut armor = plate_armor();
        assert_eq!(armor.spd_bonus, -1);
        upgrade_equipment(&mut armor);
        assert_eq!(armor.spd_bonus, -1);
    }

    #[test]
    fn test_upgrade_kit_item() {
        let kit = upgrade_kit();
        assert_eq!(kit.name, "Upgrade Kit");
        assert_eq!(kit.effect, crate::components::ItemEffect::UpgradeKit);
    }

    #[test]
    fn test_equipped_items_sum_lifesteal_specials() {
        let mut equipped = EquippedItems::default();
        equipped.equip(vampiric_ring());

        assert_eq!(equipped.total_lifesteal_percent(), 10);
        assert_eq!(equipped.total_hp_regen(), 0);
    }

    #[test]
    fn test_equipped_items_sum_hp_regen_specials() {
        let mut equipped = EquippedItems::default();
        equipped.equip(healing_wand());
        equipped.equip(divine_vestments());

        assert_eq!(equipped.total_hp_regen(), 5);
        assert_eq!(equipped.total_lifesteal_percent(), 0);
    }

    #[test]
    fn test_set_display_names() {
        assert_eq!(EquipmentSet::ShadowKnight.display_name(), "Shadow Knight");
        assert_eq!(EquipmentSet::ArcaneWeaver.display_name(), "Arcane Weaver");
        assert_eq!(
            EquipmentSet::DivineGuardian.display_name(),
            "Divine Guardian"
        );
    }

    #[test]
    fn test_set_required_items() {
        assert_eq!(
            EquipmentSet::ShadowKnight.required_items(),
            &["DarkBlade", "ShadowArmor"]
        );
        assert_eq!(
            EquipmentSet::ArcaneWeaver.required_items(),
            &["ArcaneStaff", "ArcaneRobe"]
        );
        assert_eq!(
            EquipmentSet::DivineGuardian.required_items(),
            &["DivineStaff", "DivineVestments"]
        );
    }
}
