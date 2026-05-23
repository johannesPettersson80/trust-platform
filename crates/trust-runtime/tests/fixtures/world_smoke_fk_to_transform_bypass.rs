fn fk_to_transform_bypass(scene: &mut scena::Scene, node: scena::NodeKey) -> anyhow::Result<()> {
    let fk_pose = compute_fk_for_chain();
    scene.set_transform(node, fk_pose)?;
    Ok(())
}

fn compute_fk_for_chain() -> scena::Transform {
    scena::Transform::IDENTITY
}
