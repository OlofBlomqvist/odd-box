import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';

const ReadmeViewer = () => {
  const [readmeContent, setReadmeContent] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    const fetchReadme = async () => {
      try {
        const response = await fetch('/readme.md'); // Fetch from the public folder
        if (!response.ok) {
          throw new Error('Failed to fetch README');
        }

        const content = await response.text();
        setReadmeContent(content);
      } catch (err:any) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    };

    fetchReadme();
  }, []);

  if (loading) return <p>Loading...</p>;
  if (error) return <p>Error: {error}</p>;

  return (
    <div className="prose">
      <ReactMarkdown className="[&_p]:m-[revert] [&_a]:text-[var(--color2)] [&_h3]:m-revert [&_h2]:m-revert [&_ul]:list-disc [&_ul]:p-[revert] [&_h3]:text-xl [&_h2]:text-xl [&_h3]:font-bold [&_h2]:font-bold max-w-[750px] [&_pre]:p-[10px] [&_pre]:bg-[#00000054] [&_pre]:m-[revert] [&_code]:whitespace-pre-wrap" remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]}>
        {readmeContent}
      </ReactMarkdown>
    </div>
  );
};

export default ReadmeViewer;