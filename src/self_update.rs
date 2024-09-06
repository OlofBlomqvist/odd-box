use self_update::cargo_crate_version;
use serde::Deserialize;
use std::fmt::Debug;

#[derive(Deserialize, Debug, Clone)]
struct Release {
    #[allow(dead_code)] html_url: Option<String>,
    tag_name: Option<String>,
}

fn update_from_github(target_tag:&str,current_version:&str) -> anyhow::Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("OlofBlomqvist")
        .repo_name("odd-box")
        .bin_name("odd-box")
        .show_download_progress(true)
        .target_version_tag(target_tag)
        .current_version(current_version)
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}

pub async fn update() -> anyhow::Result<()> {
    let releases_url = "https://api.github.com/repos/OlofBlomqvist/odd-box/releases";   
    let c = reqwest::Client::new();
    let latest_release: Release = c.get(releases_url).header("user-agent", "odd-box").send()
        .await
        .expect("request failed")
        .json::<Vec<Release>>()
        .await
        .expect("failed to deserialize").iter().filter(|x|{
            if let Some(t) = &x.tag_name {
                t.to_lowercase().contains("-preview") == false
            } else {
                false
            }
        }).next().unwrap().clone();

    let current_version = cargo_crate_version!();
    let latest_tag = latest_release.tag_name.unwrap();
    if format!("v{current_version}") == latest_tag {
        println!("already running latest version: {latest_tag}");
        return Ok(())
    }

    update_from_github(&latest_tag,&current_version)

}
