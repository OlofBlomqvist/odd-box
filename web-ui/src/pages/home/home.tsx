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

      <div style={{ marginTop: "20px" }}>
        <h3>ODD-BOX</h3>
        <hr />
        <p>{import.meta.env.VITE_MY_STUFF}</p>
        <div
          style={{ fontSize: ".9rem", marginTop: "10px", maxWidth: "750px" }}
        >
          <p>
            A simple to use cross-platform toy-level reverse proxy server for
            local development and tinkering purposes.
          </p>
          <br />
          <p>
            It allows you to configure a list of processes to run and host them
            behind their own custom hostnames. Automatically generates
            (self-signed) certificates for HTTPS when accessing them the first
            time (cached in .odd-box-cache dir).
          </p>
          <br />
          <p>
            Uses the 'port' environment variable to assign a port for each site.
            If your process does not support using the port environment
            variable, you can pass custom arguments or variables for your
            process instead.
          </p>
          <br />
          <p>
            You can enable or disable all sites or specific ones using the
            http://localhost/START and http://localhost/STOP endpoints,
            optionally using query parameter "?proc=my_site" to stop or start a
            specific site. (Mostly only useful for pre-build scripts where you
            dont want to manually stop and start the proxy on each rebuild.
            Sites start automatically again on the next request)
          </p>
          <br />
          <h3>Main Features & Goals</h3>
          <ul>
            <li>Cross platform (win/lin/osx)</li>
            <li>Easy to configure</li>
            <li>Keep a list of specified binaries running</li>
            <li>Uses PORT environment variable for routing</li>
            <li>Allows for setting proc specific and global env vars</li>
            <li>Remote target proxying</li>
            <li>Terminating proxy that supports both HTTP/1.1 & HTTP2</li>
            <li>TCP tunnelling for HTTP/1</li>
            <li>TCP tunnelling for HTTPS/1 via SNI sniffing</li>
            <li>TCP tunnelling for HTTP/2 over HTTP/1 (h2c upgrade)</li>
            <li>H2C via terminating proxy</li>
            <li>Automatic self-signed certs for all hosted processes </li>
          </ul>
        </div>
      </div>
    </div>
  );
};

export default HomePage;
