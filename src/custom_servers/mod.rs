pub mod directory;
use directory::*;
use markdown::{CompileOptions, Options, ParseOptions};

pub fn markdown_to_html(
    theme: ThemeDecision,
    title: &str,
    text: &str,
) -> Result<String, markdown::message::Message> {
    let mut opts = Options {
        parse: ParseOptions::gfm(),
        compile: CompileOptions {
            allow_dangerous_html: true,
            ..CompileOptions::default()
        },
    };
    opts.parse.constructs.gfm_table = true;
    opts.parse.constructs.thematic_break = true;
    opts.parse.constructs.code_text = true;

    // normalise unchecked boxes (“- []” → “- [ ]”) so GFM parses them
    let cleaned = text.replace("- [] ", "- [ ] ").replace("* [] ", "* [ ] ");

    let md_html = markdown::to_html_with_options(&cleaned, &opts)?;

    Ok(format!(
        r#"
<!doctype html>
<html lang="en" {html_tag_style}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>

<!-- === early theme bootstrap ========================================= -->
<script>
(() => {{
    /* ----- helpers ---------------------------------------------------- */
    const getCookieTheme = () => {{
        const m = document.cookie.match(/(?:^|;\s*)theme=(dark|light)/);
        return m ? m[1] : null;
    }};
    const sysPrefersDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;

    /* ----- decide ----------------------------------------------------- */
    const theme =
        getCookieTheme() ??
        localStorage.getItem('theme') ??
        (sysPrefersDark ? 'dark' : 'light');

    const dark = theme === 'dark';

    /* ----- paint & class --------------------------------------------- */
    document.documentElement.classList.add(dark ? 'dark-mode' : 'light-mode');
    document.documentElement.style.backgroundColor = dark ? '#161a29' : '#ffffff';
    document.documentElement.style.color           = dark ? '#ffffff' : '#000000';
}})();
</script>

<!-- placeholders that will be swapped by JS after boot -->
<link id="theme-stylesheet" rel="stylesheet">
<link id="syntax-stylesheet" rel="stylesheet">

<style>
:root {{
    --font-size: 1rem;
    --font-family: Arial, sans-serif;
    --line-height: 1.6;
}}
body {{
    font-family: var(--font-family);
    font-family: 'Crimson Text', serif;
    font-size: var(--font-size);
    line-height: var(--line-height);
    margin: 0;
    padding: 2rem;
    transition: background-color .3s,color .3s;
    display:flex;justify-content:center;
}}
.inner-markdown {{ max-width:1400px;width:100%;position:relative; }}
/* --- toggle ---------------------------------------------------------- */
.theme-toggle {{
    position:absolute;top:1rem;right:1rem;width:40px;height:20px;background:#ccc;
    border-radius:10px;display:flex;align-items:center;cursor:pointer;z-index:1;
}}
.toggle-slider {{
    position:absolute;width:16px;height:16px;background:#333;border-radius:50%;
    transition:transform .3s;
}}
.light-mode .toggle-slider {{ transform:translateX(2px);  }}
.dark-mode  .toggle-slider {{ transform:translateX(22px); }}
.light-mode pre {{ border:1px solid #ddd; }}
.dark-mode  pre {{ border:1px solid #333; }}
.markdown-body pre code {{ white-space:break-spaces!important; }}
.markdown-body {{
        background-color: transparent !important;
}}
</style>

<script>
/* after the bootstrap above has decided, wire up the page -------------- */
window.addEventListener('DOMContentLoaded', () => {{
    const dark = document.documentElement.classList.contains('dark-mode');

    const themeSheet   = document.getElementById('theme-stylesheet');
    const syntaxSheet  = document.getElementById('syntax-stylesheet');
    const setSheets = (isDark) => {{
        themeSheet.href  = `https://cdn.jsdelivr.net/gh/hyrious/github-markdown-css@main/dist/${{isDark?'dark':'light'}}.css`;
        syntaxSheet.href = `https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/styles/${{isDark?'atom-one-dark':'github'}}.min.css`;
    }};
    setSheets(dark);

    window.toggleTheme = () => {{
        const nowDark = document.documentElement.classList.toggle('dark-mode');
        document.documentElement.classList.toggle('light-mode', !nowDark);
        document.documentElement.style.backgroundColor = nowDark ? '#161a29' : '#ffffff';
        document.documentElement.style.color           = nowDark ? '#ffffff' : '#000000';
        setSheets(nowDark);

        /* persist */
        localStorage.setItem('theme', nowDark ? 'dark' : 'light');
        document.cookie = `theme=${{nowDark?'dark':'light'}}; path=/; SameSite=Lax`;
    }};
}});
</script>
</head>

<body class="markdown-body">
    <div class="inner-markdown">
        <div class="theme-toggle" onclick="toggleTheme()">
            <div class="toggle-slider"></div>
        </div>
        {md_html}
    </div>

    <!-- highlight.js ---------------------------------------------------- -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/highlight.min.js"></script>
    <script>
        hljs.highlightAll();
    </script>
</body>
</html>
"#,
        title           = title,
        md_html         = md_html,
        /* server‑side colour flash‑guard */
        html_tag_style  = match theme {
            ThemeDecision::Dark  => "style='background:#161a29;color:#ffffff'",
            ThemeDecision::Light => "style='background:#ffffff;color:#000000'",
            ThemeDecision::Auto  => "",
        },
    ))
}
