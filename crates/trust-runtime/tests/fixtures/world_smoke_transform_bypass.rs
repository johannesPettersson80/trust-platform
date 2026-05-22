fn bypass_dynamic_body_transform_write(scene: &mut scena::Scene, node: scena::NodeKey) {
    scene
        .set_transform(node, scena::Transform::IDENTITY)
        .expect("bypass fixture must be rejected by lint before compilation");
}
