import "./footer-style.css";
const Footer = () => {
  return (
    <footer className="odd-footer">
      <div className="max-w-[900px] flex justify-between">

      <div className="flex flex-col gap-2">

      <div className="flex items-center gap-2">
        <span>
          <img src="/ob3.png" className="h-14"/>
        </span>
        {/* <p>ODD-BOX</p> */}
        <span className="text-sm">
          Reverse proxying made easy
        </span>
      </div>
        <p className="text-sm mt-2 opacity-60">A simple, cross-platform reverse proxy server.<br/>Tailored for local development and tinkering.</p>
      </div>
      <div className="flex flex-col gap-2">
        <p className="text-lg font-bold">About</p>
        <a target="_blank" rel="noopener noreferrer" href="https://github.com/OlofBlomqvist/odd-box" className="text-sm opacity-90 cursor-pointer hover:underline">About odd-box</a>
        <a target="_blank" rel="noopener noreferrer" href="https://github.com/OlofBlomqvist/odd-box/releases" className="text-sm opacity-90 cursor-pointer hover:underline">Change log</a>
        <a target="_blank" rel="noopener noreferrer" href="https://github.com/OlofBlomqvist/odd-box/releases" className="text-sm opacity-90 cursor-pointer hover:underline">Releases</a>
      </div>
      </div>
    </footer>
  );
};

export default Footer;
