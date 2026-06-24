fn main() {
    embed_resource::compile("assets/icons/gksrmf.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
