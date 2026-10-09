    use tower_lsp::lsp_types::{Position as HmiPosition, TextEdit as HmiTextEdit};
    use trust_ide::rename_identity::RenameIdentity;

    const HMI_RENAME_PROGRAM: &str = "FUNCTION_BLOCK Pump\nVAR_INPUT\n    Start : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\n    Running := Start;\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P101 : Pump;\n    Pumps : ARRAY[1..2] OF Pump;\n    Running : BOOL;\nEND_VAR\n    P101(Start := TRUE);\nEND_PROGRAM\n";
    const HMI_RENAME_PAGE: &str = "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"pump\"\ntype = \"status\"\nat = [0, 0]\nsize = [240, 48]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n\n[[object]]\nid = \"second\"\ntype = \"value\"\nat = [0, 60]\nsize = [240, 96]\n\n[object.bind]\nvalue = \"Main.Pumps[2].Running\"\n\n[[object]]\nid = \"local\"\ntype = \"status\"\nat = [0, 180]\nsize = [240, 48]\n\n[object.bind]\nstate = { path = \"Main.Running\", direction = \"read\" }\n";
    const HMI_RENAME_POPUP: &str = "format = 1\ntitle = \"Pump\"\nsize = [320, 200]\n\n[[object]]\nid = \"state\"\ntype = \"status\"\nat = [0, 0]\nsize = [240, 48]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n";
    const HMI_RENAME_BROKEN: &str = "format = 1\ntitle = 3\n\n[[object]]\nid = \"x\"\ntype = \"status\"\nat = [0, 0]\nsize = [240, 48]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n";

    /// A project with a program, a page, a popup and a page that does not load.
    fn hmi_rename_project(name: &str) -> (PathBuf, MockContext, Url) {
        let root = temp_dir(name);
        for folder in ["src", "hmi/pages", "hmi/popups"] {
            std::fs::create_dir_all(root.join(folder)).expect("create a folder");
        }
        std::fs::write(root.join("src/main.st"), HMI_RENAME_PROGRAM).expect("write source");
        std::fs::write(root.join("hmi/pages/overview.toml"), HMI_RENAME_PAGE).expect("write page");
        std::fs::write(root.join("hmi/popups/pump.toml"), HMI_RENAME_POPUP).expect("write popup");
        std::fs::write(root.join("hmi/pages/broken.toml"), HMI_RENAME_BROKEN).expect("write page");
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(root_uri.clone(), test_project_config(root.to_string_lossy().as_ref(), "x86_64"))],
            workspace_folders: vec![root_uri],
            ..MockContext::default()
        };
        let document = Url::from_file_path(root.join("src/main.st")).expect("source uri");
        (root, context, document)
    }

    fn hmi_file(root: &std::path::Path, file: &str) -> Url {
        Url::from_file_path(root.join(file)).expect("file uri")
    }

    /// The text after the edits, which may not overlap.
    fn with_edits(text: &str, edits: &[HmiTextEdit]) -> String {
        let offset = |position: HmiPosition| -> usize {
            let line_start: usize = text.split_inclusive('\n').take(position.line as usize).map(str::len).sum();
            let line = &text[line_start..];
            let mut units = 0;
            for (index, character) in line.char_indices() {
                if units >= position.character as usize {
                    return line_start + index;
                }
                units += character.len_utf16();
            }
            line_start + line.len()
        };
        let mut sorted: Vec<&HmiTextEdit> = edits.iter().collect();
        sorted.sort_by_key(|edit| std::cmp::Reverse((edit.range.start.line, edit.range.start.character)));
        let mut result = text.to_string();
        for edit in sorted {
            result.replace_range(offset(edit.range.start)..offset(edit.range.end), &edit.new_text);
        }
        result
    }

    #[test]
    fn hmi_rename_renames_a_member_in_every_binding_that_names_it() {
        let (root, context, document) = hmi_rename_project("trustlsp-hmi-rename-member");
        let identity = RenameIdentity::Member { owner: "Pump".into(), name: "Running".into() };
        let changes = hmi_rename::rename_edits_with_context(&context, &document, &identity, "IsRunning").expect("valid rename");
        let page = changes.get(&hmi_file(&root, "hmi/pages/overview.toml")).expect("the page's edits");
        assert_eq!(
            with_edits(HMI_RENAME_PAGE, page),
            HMI_RENAME_PAGE
                .replace("\"Main.P101.Running\"", "\"Main.P101.IsRunning\"")
                .replace("\"Main.Pumps[2].Running\"", "\"Main.Pumps[2].IsRunning\""),
            "the pumps' members change; Main's own Running and every other byte stay"
        );
        let popup = changes.get(&hmi_file(&root, "hmi/popups/pump.toml")).expect("the popup's edits");
        assert_eq!(with_edits(HMI_RENAME_POPUP, popup), HMI_RENAME_POPUP.replace("Main.P101.Running", "Main.P101.IsRunning"));
        assert!(!changes.contains_key(&hmi_file(&root, "hmi/pages/broken.toml")), "a file that does not load is not changed");
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_rename_renames_a_program_as_the_first_part_of_its_paths() {
        let (root, context, document) = hmi_rename_project("trustlsp-hmi-rename-program");
        let identity = RenameIdentity::Program { name: "Main".into() };
        let changes = hmi_rename::rename_edits_with_context(&context, &document, &identity, "Plant").expect("valid rename");
        let page = changes.get(&hmi_file(&root, "hmi/pages/overview.toml")).expect("the page's edits");
        assert_eq!(with_edits(HMI_RENAME_PAGE, page), HMI_RENAME_PAGE.replace("\"Main.", "\"Plant."));
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn hmi_references_lists_each_binding_that_names_a_member_at_its_line() {
        let (root, context, document) = hmi_rename_project("trustlsp-hmi-references");
        let identity = RenameIdentity::Member { owner: "Pump".into(), name: "Running".into() };
        let mut found: Vec<(String, u32, u32, u32)> = hmi_rename::references_with_context(&context, &document, &identity)
            .into_iter()
            .map(|location| {
                let file = location.uri.path().rsplit("/hmi/").next().unwrap_or_default().to_string();
                (file, location.range.start.line, location.range.start.character, location.range.end.character)
            })
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                ("pages/overview.toml".to_string(), 10, 19, 26),
                ("pages/overview.toml".to_string(), 19, 23, 30),
                ("popups/pump.toml".to_string(), 11, 19, 26),
            ],
            "the word Running on each binding's line"
        );
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }


    /// Specification 39, §7.8: an equipment object's instance follows a rename, and so does its
    /// displayed tag while that is the instance's own name; Find All References lists the instances.
    #[test]
    fn renaming_an_instance_renames_the_objects_that_show_it() {
        let (root, context, document) = hmi_rename_project("trustlsp-hmi-rename-equipment");
        let page = "format = 1\ntitle = \"Equipment\"\n\n[[object]]\nid = \"pump-1\"\ntag = \"P101\"\ntype = \"status\"\nblock = \"Pump\"\ninstance = \"Main.P101\"\nat = [0, 0]\nsize = [40, 40]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n\n[[object]]\nid = \"pump-2\"\ntag = \"P-101\"\ntype = \"status\"\nblock = \"Pump\"\ninstance = \"Main.P101\"\nat = [60, 0]\nsize = [40, 40]\n";
        std::fs::write(root.join("hmi/pages/equipment.toml"), page).expect("write page");
        let identity = RenameIdentity::Member { owner: "Main".into(), name: "P101".into() };
        let changes = hmi_rename::rename_edits_with_context(&context, &document, &identity, "FeedPump").expect("rename");
        let edited = with_edits(page, &changes[&hmi_file(&root, "hmi/pages/equipment.toml")]);
        assert_eq!(
            edited,
            page.replace("tag = \"P101\"", "tag = \"FeedPump\"").replace("Main.P101", "Main.FeedPump"),
            "both instances and the binding change; the tag P-101 is the object's own"
        );
        let lines: Vec<u32> = hmi_rename::references_with_context(&context, &document, &identity)
            .into_iter()
            .filter(|location| location.uri == hmi_file(&root, "hmi/pages/equipment.toml"))
            .map(|location| location.range.start.line)
            .collect();
        assert_eq!(lines, vec![13, 8, 20], "the binding and both instances, not the tag");
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    /// Specification 39, §10.4: renaming an instance moves its design choices in the same edit.
    #[test]
    fn renaming_an_instance_moves_its_design_choices_in_the_same_edit() {
        let (root, context, document) = hmi_rename_project("trustlsp-plant-rename-design");
        let design = "format = 2\n\n[equipment.\"Main.P101\"]\nblock = \"Pump\"\n\n[equipment.\"Main.P101\".settings]\nStartTimeout = \"T#10s\"\n";
        std::fs::write(root.join("plant.toml"), design).expect("design");
        let identity = RenameIdentity::Member { owner: "Main".into(), name: "P101".into() };
        let changes = hmi_rename::rename_edits_with_context(&context, &document, &identity, "FeedPump").expect("rename");
        let edited = with_edits(design, &changes[&hmi_file(&root, "plant.toml")]);
        let parsed = super::hmi_plant_project::PlantDesign::parse(&edited).expect("renamed design");
        assert_eq!(parsed.entry("Main.FeedPump").expect("moved").settings["StartTimeout"], "T#10s");
        assert!(parsed.entry("Main.P101").is_none());
        assert!(changes.contains_key(&hmi_file(&root, "hmi/pages/overview.toml")));
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
