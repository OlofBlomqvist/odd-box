use anyhow::bail;
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

    let current_version = current_version();
    let latest_tag = find_latest_version(false).await?;
    if format!("v{current_version}") == latest_tag {
        println!("already running latest version: {latest_tag}");
        return Ok(())
    }

    update_from_github(&latest_tag,&current_version)

}

pub fn current_version() -> &'static str { cargo_crate_version!() }

/// returns Some(newer_version) or None if current is latest.
// pub async fn current_is_latest() -> anyhow::Result<Option<String>> {
//     let current_version = current_version();
//     match find_latest_version(false).await {
//         Ok(v) if current_version != v => Ok(Some(v)),
//         Ok(_) => Ok(None),
//         Err(e) => Err(e)
//     }
// }


pub async fn find_latest_version(include_pre:bool) -> anyhow::Result<String> {

    let allow_preview = include_pre ||
        std::env::vars()
            .find(|(key,_)| key=="ODDBOX_ALLOW_PREVIEW").map(|x|x.1.to_lowercase())
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");

    let releases_url = "https://api.github.com/repos/OlofBlomqvist/odd-box/releases";   
    let c = reqwest::Client::new();
    let latest_release_tag: Option<String> = c.get(releases_url).header("user-agent", "odd-box").send()
        .await?
        .json::<Vec<Release>>()
        .await
        ?.iter().filter(|x|{
            if let Some(t) = &x.tag_name {
                allow_preview ||
                    t.to_lowercase().contains("-preview") == false
                    && t.to_lowercase().contains("-alpha") == false
                    && t.to_lowercase().contains("-beta") == false
                    && t.to_lowercase().contains("-rc") == false
            } else {
                false
            }
        }).next().map(|x|x.tag_name.clone()).unwrap_or(None);


    if let Some(v) = latest_release_tag {
        Ok(v.clone())
    } else {
        bail!("Failed to find latest release version")
    }
   

}