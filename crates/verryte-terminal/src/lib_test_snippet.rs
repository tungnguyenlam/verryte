    #[test]
    fn test_visual_registry() {
        let mut registry = VisualRegistry::new();
        registry.register_single_cell("test", '@', Color::RED, Color::BLUE);

        let grid = Grid::new(2, 2);
        registry.register("block", VisualAsset::BlockSprite(grid.clone()));

        if let Some(VisualAsset::SingleCell(g)) = registry.get("test") {
            let cell = g.get(0, 0).unwrap();
            assert_eq!(cell.glyph, '@');
            assert_eq!(cell.fg, Color::RED);
            assert_eq!(cell.bg, Color::BLUE);
        } else {
            panic!("Expected SingleCell");
        }

        if let Some(VisualAsset::BlockSprite(g)) = registry.get("block") {
            assert_eq!(g.width(), 2);
            assert_eq!(g.height(), 2);
        } else {
            panic!("Expected BlockSprite");
        }
    }
