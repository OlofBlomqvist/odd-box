const Footer = () => {
  return (
    <footer className="fixed bottom-0 left-0 right-0 h-[180px] z-[-1] bg-[#1f1f1f] flex justify-between py-5 font-light ml:pl-[320px]">
      <div className="max-w-[900px] grid gap-2 grid-cols-[1fr_1px_1fr] justify-between w-full px-[20px] justify-items-center">
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <span>
              <img src="/ob3.png" className="h-14 min-w-14" />
            </span>
            <span className="text-sm">Reverse proxying made easy</span>
          </div>
          <p className="text-sm mt-2 opacity-60">
            A simple, cross-platform reverse proxy server.
            <br />
            Tailored for local development and tinkering.
          </p>
        </div>
        <div className="bg-[#ffffff24] w-[1px] h-[100px]"/>
        <div className="flex flex-col gap-2">
          <p className="text-lg font-bold">About</p>
          <a
            target="_blank"
            rel="noopener noreferrer"
            href="https://github.com/OlofBlomqvist/odd-box"
            className="text-sm opacity-90 cursor-pointer hover:underline"
          >
            About odd-box
          </a>
          <a
            target="_blank"
            rel="noopener noreferrer"
            href="https://github.com/OlofBlomqvist/odd-box/releases"
            className="text-sm opacity-90 cursor-pointer hover:underline"
          >
            Change log
          </a>
          <a
            target="_blank"
            rel="noopener noreferrer"
            href="https://github.com/OlofBlomqvist/odd-box/releases"
            className="text-sm opacity-90 cursor-pointer hover:underline"
          >
            Releases
          </a>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
