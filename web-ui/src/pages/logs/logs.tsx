import SiteLogs from "../site/site-logs";

const LogsPage = () => {
  return (
    <main className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]">
      <SiteLogs />
    </main>
  );
};

export default LogsPage;
