use markdown::{CompileOptions, Options, ParseOptions};

pub mod directory;


pub fn markdown_to_html(title: &str, text: &str) -> Result<String, markdown::message::Message> {
    let mut mo = Options {
        parse: ParseOptions::gfm(),
        compile: CompileOptions {
          allow_dangerous_html: true,
          ..CompileOptions::default()
        }
    };
    mo.parse.constructs.gfm_table = true;
    mo.parse.constructs.thematic_break = true;
    mo.compile.allow_dangerous_html = true;
    mo.parse.constructs.code_text = true;
    
    let html = markdown::to_html_with_options(&text, &mo)?;

    Ok(format!(
        r#"<!DOCTYPE html>
        <html lang="en">
        <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>{title}</title>
        
        <!-- Load initial markdown theme and syntax highlighting CSS based on user preference -->
        <link id="theme-stylesheet" rel="stylesheet" href="">
        <link id="syntax-stylesheet" rel="stylesheet" href="">

        <style>
        /* Base styling and theme switch */
        :root {{
            --font-size: 1rem;
            --font-family: Arial, sans-serif;
            --line-height: 1.6;
        }}

        body {{
            font-family: var(--font-family);
            font-size: var(--font-size);
            line-height: var(--line-height);
            padding: 2rem;
            transition: background-color 0.3s, color 0.3s;
            display: flex;
            justify-content: center;
        }}
        
        .inner-markdown {{
            max-width: 1400px;
            margin: 0 auto;
            position: relative; /* Position for absolute toggle button */
        }}
        
        /* Toggle Switch */
        .theme-toggle {{
            position: absolute;
            top: 1rem;
            right: 1rem;
            width: 40px;
            height: 20px;
            background-color: #ccc;
            border-radius: 10px;
            display: flex;
            align-items: center;
            cursor: pointer;
            z-index: 1;
        }}
        .toggle-slider {{
            position: absolute;
            width: 16px;
            height: 16px;
            background-color: #333;
            border-radius: 50%;
            transition: transform 0.3s;
        }}
        .light-mode .toggle-slider {{
            transform: translateX(2px);
        }}
        .dark-mode .toggle-slider {{
            transform: translateX(22px);
        }}
        .light-mode pre {{
            border: 1px solid #ddd;
        }}
        .dark-mode pre {{
            border: 1px solid #333;
        }}
        .markdown-body pre code {{
            white-space: break-spaces !important;
        }}
        </style>
        <script>
            // Set initial theme based on user preference or system setting
            const userPrefersDark = localStorage.getItem('theme') === 'dark' || 
                (!localStorage.getItem('theme') && window.matchMedia('(prefers-color-scheme: dark)').matches);
            const initialTheme = userPrefersDark ? 'dark' : 'light';
            document.documentElement.classList.add(userPrefersDark ? 'dark-mode' : 'light-mode');

            // Set the initial theme and syntax highlighting stylesheets
            const themeStylesheet = document.getElementById('theme-stylesheet');
            const syntaxStylesheet = document.getElementById('syntax-stylesheet');
            themeStylesheet.href = `https://cdn.jsdelivr.net/gh/hyrious/github-markdown-css@main/dist/${{initialTheme}}.css`;
            syntaxStylesheet.href = `https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/styles/${{userPrefersDark ? 'atom-one-dark' : 'github'}}.min.css`;

            // Toggle theme function
            function toggleTheme() {{
                document.documentElement.classList.toggle('dark-mode');
                document.documentElement.classList.toggle('light-mode');
                const isDarkMode = document.documentElement.classList.contains('dark-mode');
                
                // Swap the markdown theme and syntax highlighting stylesheets based on the current theme
                themeStylesheet.href = `https://cdn.jsdelivr.net/gh/hyrious/github-markdown-css@main/dist/${{isDarkMode ? 'dark' : 'light'}}.css`;
                syntaxStylesheet.href = `https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/styles/${{isDarkMode ? 'atom-one-dark' : 'github'}}.min.css`;

                // Store the preference
                localStorage.setItem('theme', isDarkMode ? 'dark' : 'light');
            }}
        </script>
        </head>
        <body class='markdown-body'>
            <div class='inner-markdown'>
                <div class="theme-toggle" onclick="toggleTheme()">
                    <div class="toggle-slider"></div>
                </div>
                {html}
            </div>
        
        <!-- Load Highlight.js library -->
        <script src='https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/highlight.min.js'></script>
        <script>
            // Apply syntax highlighting to all code blocks
            document.querySelectorAll('code[class^="language-"]').forEach((el) => {{
                hljs.highlightElement(el);
            }});
        </script>
        </body>
        </html>
        "#,
        title = title,
        html = html,
    ))
}
