#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_namespace_updates_using_and_qualified_names() {
        let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
FUNCTION FooFunc : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    USING LibA;
    VAR
        x : LibA.Foo;
    END_VAR
    x := LibA.FooFunc();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let old_path = parse_namespace_path("LibA").expect("old path");
        let new_path = parse_namespace_path("Company.LibA").expect("new path");

        let result = move_namespace_path(&db, &old_path, &new_path).expect("rename result");
        let edits = result.edits.get(&file_id).expect("file edits");

        let using_edit = edits
            .iter()
            .find(|edit| edit.new_text == "Company.LibA")
            .expect("using edit");
        let using_start: usize = using_edit.range.start().into();
        let using_end: usize = using_edit.range.end().into();
        assert!(source[using_start..using_end].contains("LibA"));

        assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA.Foo"));

        let qualified_edit = edits
            .iter()
            .find(|edit| edit.new_text == "Company.LibA.FooFunc")
            .expect("field expr edit");
        let qualified_start: usize = qualified_edit.range.start().into();
        let qualified_end: usize = qualified_edit.range.end().into();
        assert!(source[qualified_start..qualified_end].contains("LibA.FooFunc"));
    }

    #[test]
    fn generate_interface_stubs_inserts_missing_members() {
        let source = r#"
INTERFACE IControl
    METHOD Start
    END_METHOD

    PROPERTY Status : INT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE

CLASS Pump IMPLEMENTS IControl
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("IMPLEMENTS IControl").expect("implements");
        let result =
            generate_interface_stubs(&db, file_id, TextSize::from(offset as u32)).expect("stubs");
        let edits = result.edits.get(&file_id).expect("file edits");
        let insert = edits
            .iter()
            .find(|edit| !edit.new_text.is_empty())
            .expect("insert edit");
        assert!(insert.new_text.contains("METHOD PUBLIC Start"));
        assert!(insert.new_text.contains("PROPERTY PUBLIC Status"));
    }

    #[test]
    fn inline_variable_with_literal_initializer() {
        let source = r#"
PROGRAM Test
    VAR
        x : INT := 1 + 2;
    END_VAR
    y := x;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("x;").expect("x ref");
        let result = inline_symbol(&db, file_id, TextSize::from(offset as u32)).expect("inline");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text.contains("1 + 2")));
        assert!(edits.iter().any(|edit| edit.new_text.is_empty()));
    }

    #[test]
    fn inline_constant_across_files() {
        let constants = r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    ANSWER : INT := 42;
END_VAR
END_CONFIGURATION
"#;
        let program = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
VAR_EXTERNAL CONSTANT
    ANSWER : INT;
END_VAR
    x := ANSWER;
END_PROGRAM
"#;
        let mut db = Database::new();
        let const_id = FileId(0);
        let prog_id = FileId(1);
        db.set_source_text(const_id, constants.to_string());
        db.set_source_text(prog_id, program.to_string());

        let offset = program.find("ANSWER").expect("constant ref");
        let target = resolve_target_at_position(&db, prog_id, TextSize::from(offset as u32))
            .expect("target");
        let ResolvedTarget::Symbol(symbol_id) = target else {
            panic!("expected symbol target");
        };
        let symbols = db.file_symbols_with_project(prog_id);
        let symbol = symbols.get(symbol_id).expect("symbol");
        let origin = symbol.origin.expect("origin");
        let decl_file_id = origin.file_id;
        assert_eq!(decl_file_id, const_id);
        let decl_source = db.source_text(decl_file_id);
        let decl_root = parse(&decl_source).syntax();
        let decl_range = db
            .file_symbols(origin.file_id)
            .get(origin.symbol_id)
            .map(|sym| sym.range)
            .unwrap_or(symbol.range);
        let var_decl =
            crate::var_decl::find_var_decl_for_range(&decl_root, decl_range).expect("var decl");
        let expr = initializer_expr_in_var_decl(&var_decl).expect("initializer");
        let expr_info =
            inline_expr_info(&db, decl_file_id, &decl_source, &decl_root, &expr).expect("expr");
        assert!(expr_info.is_const_expr);
        let references = find_references(
            &db,
            prog_id,
            TextSize::from(offset as u32),
            FindReferencesOptions {
                include_declaration: false,
            },
        );
        assert!(!references.is_empty(), "references");

        let result = inline_symbol(&db, prog_id, TextSize::from(offset as u32)).expect("inline");
        let const_edits = result.edits.edits.get(&const_id).expect("const edits");
        let prog_edits = result.edits.edits.get(&prog_id).expect("program edits");
        assert!(prog_edits.iter().any(|edit| edit.new_text == "42"));
        assert!(const_edits.iter().any(|edit| edit.new_text.is_empty()));
    }

    #[test]
    fn inline_variable_initialized_from_var_temp_constant() {
        let source = r#"
FUNCTION_BLOCK Fb
VAR_TEMP
    CONSTANT T : INT := INT#7;
END_VAR
VAR
    Alias : INT := T;
END_VAR
VAR_OUTPUT
    X : INT;
END_VAR
    X := Alias;
END_FUNCTION_BLOCK
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.rfind("Alias;").expect("inline target ref");
        let result = inline_symbol(&db, file_id, TextSize::from(offset as u32)).expect("inline");
        assert_eq!(result.kind, InlineTargetKind::Variable);
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text == "T"));
    }

    #[test]
    fn inline_variable_initialized_from_var_input_constant() {
        let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT
    CONSTANT A : INT;
END_VAR
VAR
    Alias : INT := A;
END_VAR
VAR_OUTPUT
    X : INT;
END_VAR
    X := Alias;
END_FUNCTION_BLOCK
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.rfind("Alias;").expect("inline target ref");
        let result = inline_symbol(&db, file_id, TextSize::from(offset as u32)).expect("inline");
        assert_eq!(result.kind, InlineTargetKind::Variable);
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text == "A"));
    }

    #[test]
    fn extract_method_creates_method_and_call() {
        let source = r#"
CLASS Controller
    METHOD Run
        VAR
            x : INT;
            y : INT;
        END_VAR
        x := 1;
        y := x + 1;
    END_METHOD
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x := 1;").expect("start");
        let end = source.find("y := x + 1;").expect("end") + "y := x + 1;".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_method(&db, file_id, range).expect("extract method");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("METHOD ExtractedMethod")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_IN_OUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedMethod(x := x")));
    }

    #[test]
    fn extract_property_creates_property() {
        let source = r#"
CLASS Controller
    VAR
        speed : INT;
    END_VAR
    METHOD Run
        VAR
            x : INT;
        END_VAR
        x := speed + 1;
    END_METHOD
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("speed + 1").expect("start");
        let end = start + "speed + 1".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_property(&db, file_id, range).expect("extract property");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("PROPERTY ExtractedProperty")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedProperty := speed + 1")));
    }

    #[test]
    fn extract_pou_creates_function() {
        let source = r#"
PROGRAM Main
    VAR
        x : INT;
        y : INT;
    END_VAR
    x := 1;
    y := x + 1;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x := 1;").expect("start");
        let end = source.find("y := x + 1;").expect("end") + "y := x + 1;".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_pou(&db, file_id, range).expect("extract pou");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION ExtractedFunction")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_IN_OUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction(x := x")));
    }

    #[test]
    fn extract_pou_expression_infers_return_type() {
        let source = r#"
PROGRAM Main
    VAR
        x : INT;
        y : INT;
    END_VAR
    y := x + 1;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x + 1").expect("start");
        let end = start + "x + 1".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_pou(&db, file_id, range).expect("extract pou");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION ExtractedFunction : INT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction := x + 1")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction(x := x)")));
    }

    #[test]
    fn convert_function_to_function_block_updates_calls() {
        let source = r#"
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION

PROGRAM Main
    Foo();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION Foo").expect("function");
        let result =
            convert_function_to_function_block(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION_BLOCK")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_OUTPUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance")));
    }

    #[test]
    fn convert_function_to_function_block_updates_expression_calls() {
        let source = r#"
NAMESPACE LibA
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := LibA.Foo();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION Foo").expect("function");
        let result =
            convert_function_to_function_block(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR") && edit.new_text.contains("LibA.Foo")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance.result")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance(")));
    }

    #[test]
    fn convert_function_block_to_function_updates_signature() {
        let source = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION_BLOCK Fb").expect("function block");
        let result =
            convert_function_block_to_function(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text.contains("FUNCTION")));
        assert!(edits.iter().any(|edit| edit.new_text.contains(": INT")));
    }

    #[test]
    fn convert_function_block_to_function_requires_no_instances() {
        let fb = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;
        let program = r#"
PROGRAM Main
    VAR
        fb : Fb;
    END_VAR
    fb();
END_PROGRAM
"#;
        let mut db = Database::new();
        let fb_id = FileId(0);
        let program_id = FileId(1);
        db.set_source_text(fb_id, fb.to_string());
        db.set_source_text(program_id, program.to_string());

        let offset = fb.find("FUNCTION_BLOCK Fb").expect("function block");
        let result = convert_function_block_to_function(&db, fb_id, TextSize::from(offset as u32));
        assert!(result.is_none(), "expected conversion to be unavailable");
    }
}
