/// Query metadata owns imported shapes after the caller changes or drops
/// the full signature map; exporting another module still keeps every table.
#[test]
fn imported_queries_survive_input_mutation_and_drop() {
    let library = Typed::new(
        r#"package shapes
        type Config<T> { value: T, enabled: Bool = true }
        type Node { next: Option<Node> }
        enum Chain<T> { End, Link(T, Chain<T>) }
        type ConfigAlias<T> = Config<T>
        const ANSWER: Int = 42
        fn identity<T>(value: T) -> T = value
        protocol Transform {
          fn identity<T>(value: T) -> T = value
        }
        "#,
    );
    let symbols = check_module(&library.program, &Modules::new()).unwrap();
    let producer = type_module(&library.program, &symbols, &TypedModules::new()).unwrap();
    let exports = producer.exports();
    assert!(exports.fns.contains_key("identity"));
    assert!(exports.protocols.contains_key("Transform"));
    assert!(exports.aliases.contains_key("ConfigAlias"));
    assert_eq!(exports.const_values["ANSWER"], 42);
    assert_eq!(exports.param_names["identity"], ["value"]);
    let mut imports = TypedModules::from([("shapes".to_string(), exports)]);
    let consumer = Typed::new(
        "import shapes.{Config, Node, Chain, ConfigAlias, ANSWER, identity, Transform}\n\
         fn run(config: Config<Int>) -> Int = identity(config.value) + ANSWER\n",
    );
    let symbols = check_module(
        &consumer.program,
        &Modules::from([("shapes".to_string(), symbols)]),
    )
    .unwrap();
    let typed = type_module(&consumer.program, &symbols, &imports).unwrap();
    let imported = imports.get_mut("shapes").unwrap();
    imported.records.clear();
    imported.enums.clear();
    imported.defaults.clear();
    imported.generics.clear();
    drop(imports);
    drop(producer);

    let q = |name| QualName::new("shapes", name);
    assert_eq!(
        typed.record_fields(&q("Config")).unwrap(),
        &[
            ("value".to_string(), Ty::Param("T".to_string())),
            ("enabled".to_string(), Ty::Bool),
        ]
    );
    assert!(typed.field_has_default(&q("Config"), "enabled"));
    assert!(!typed.field_has_default(&q("Config"), "value"));
    assert_eq!(
        typed.enum_variants(&q("Chain")).unwrap()[0],
        ("End".to_string(), vec![])
    );
    for name in ["Config", "Chain", "identity"] {
        assert_eq!(typed.type_params(&q(name)).unwrap(), ["T"]);
    }
    assert_eq!(
        typed.role_type_params(&q("Transform"), "identity").unwrap(),
        ["T"]
    );
    let chain = Ty::GenericEnum(q("Chain"), vec![Ty::Text]);
    assert_eq!(
        typed.variant(&chain, "Link"),
        Some((1, vec![Ty::Text, chain.clone()]))
    );
    for ty in [
        chain,
        Ty::Record(q("Node")),
        Ty::GenericRecord(q("Config"), vec![Ty::Int]),
    ] {
        assert_eq!(typed.equality(&ty), Some(ignite_types::Equality::Structural));
    }
    assert!(typed.boxed_payload(&q("Chain"), 1, 1));
    assert!(!typed.boxed_payload(&q("Chain"), 1, 0));
    assert!(typed.boxed_field(&q("Node"), "next"));
    let call = consumer.find("identity(config.value)");
    assert_eq!(typed.type_of(call), &Ty::Int);
    assert_eq!(typed.instance(call), Some(&[Ty::Int][..]));
    assert!(typed.exports().fns.contains_key("run"));
}
