fn main() {
    println!("cargo:rerun-if-env-changed=KEKBULL_ICHOR_PROGRAM_ID");
    if std::env::var_os("KEKBULL_ICHOR_PROGRAM_ID").is_none() {
        println!(
            "cargo:warning=KEKBULL_ICHOR_PROGRAM_ID is unset. This crate has no \
             placeholder program ID; supply the deploy-time identity at build."
        );
    }
}
