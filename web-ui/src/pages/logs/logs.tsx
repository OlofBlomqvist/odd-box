import SiteLogs from "../site/site-logs";

const LogsPage = () => {
  return (
    <>
      <p
        style={{
          textTransform: "uppercase",
          fontSize: ".9rem",
          fontWeight: "bold",
          color: "var(--color2)",
        }}
      >
        Logs
      </p>
      <SiteLogs />
    </>
  );
};

export default LogsPage;
