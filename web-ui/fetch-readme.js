// fetchReadme.js
import { writeFileSync } from 'fs';

async function fetchReadme() {
  const url = `https://api.github.com/repos/OlofBlomqvist/odd-box/readme`;

  try {
    const response = await fetch(url, {
      headers: {
        Accept: 'application/vnd.github.v3.raw', // Get the raw content
      },
    });

    if (!response.ok) {
      throw new Error('Failed to fetch README');
    }

    const readmeContent = await response.text();

    // Save to a static file
    writeFileSync('public/readme.md', readmeContent);
    console.log('README fetched and saved to public/readme.md');
  } catch (error) {
    console.error(error);
  }
}

fetchReadme();
