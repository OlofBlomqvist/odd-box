import ReadmeViewer from "@/components/readme_viewer/readme_viewer";
import "./home-styles.css";
const HomePage = () => {
  return (
    <div style={{ paddingBottom: "50px" }}>
      <p
        style={{
          textTransform: "uppercase",
          fontSize: ".9rem",
          fontWeight: "bold",
          color: "var(--color2)",
        }}
      >
        Home
      </p>
<ReadmeViewer />
      {/* <div style={{ marginTop: "20px" }}>
        <h3>ODD-BOX</h3>
        <hr />
        <div
          style={{ fontSize: ".9rem", marginTop: "10px", maxWidth: "750px" }}
        >
          <p>
          A simple, cross-platform reverse proxy server tailored for local development and tinkering. Think of it as a lightweight (and more streamlined) alternative to something like IIS, but with a key difference: configuration is primarily done declaratively through structured files, rather than a graphical user interface.
          </p>
          <br />
          <p>
          It allows you to configure a list of processes to run and host them behind their own custom hostnames. Self-signed certificates for HTTPS are automatically generated when accessing a site thru the terminating proxy service the first time (cached in .odd-box-cache dir). As with most reverse-proxy servers, odd-box also supports targetting remote backend servers.
          </p>
          <br />
          <p>
          As configuration is done thru basic files (toml format) which are easy to share, it's very easy to reproduce a particular setup.
          </p>
          <br />
          <p>
          Pre-built binaries are available in the <a className="text-[var(--color5)] hover:underline" href="https://github.com/OlofBlomqvist/odd-box/releases">release section</a>.
          </p>
          <br />
          <p>
          You can also build it yourself, or install it using brew, cargo, nix or devbox; see the installation section for guidance.
          </p>
          <br/>
          <h3 className="text-xl">Features</h3>
          <ul className="list-disc p-[revert]">
          <li>Cross platform (win/lin/osx)</li>
<li>Easy to configure (toml files)</li>
<li>Keep a list of specified binaries running</li>
<li>Uses PORT environment variable for routing</li>
<li>Allows for setting proc specific and global env vars</li>
<li>Remote target proxying</li>
<li>Terminating proxy that supports both HTTP/1.1 &amp; HTTP2</li>
<li>TCP tunnelling for HTTP/1</li>
<li>TCP tunnelling for HTTPS/1 &amp; HTTP2 via SNI sniffing</li>
<li>TCP tunnelling for HTTP/2 over HTTP/1 (h2c upgrade)</li>
<li>H2C via terminating proxy</li>
<li>Automatic self-signed certs for all hosted processes</li>
<li>Basic round-robin loadbalancing for remote targets</li>
<li>Terminating proxy supports automaticly generating lets-encrypt certificates</li>
          </ul>
<br/>
<h3 className="text-xl">Performance</h3>
<p>While the goal of this project is not to provide a state-of-the-art level performing proxy server for production environments, but rather a tool for simplifying local development scenarios, we do try to keep performance in mind be blazingly fast :-) Seriously though, performance is actually pretty good but it is not a priority (yet).</p>





        </div>
      </div> */}
    </div>
  );
};

export default HomePage;
