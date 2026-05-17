use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let arg = std::env::args().nth(1).context("usage: ctx <count>")?;
    let n = ctx::parse_count(&arg).context("parsing count")?;
    println!("{n}");
    Ok(())
}
