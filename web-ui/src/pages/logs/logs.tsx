import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import SiteLogs from "../site/site-logs";

const LogsPage = () => {
  return (
    <main className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]">
      <Card>
        <CardHeader>
          <CardTitle>Logs</CardTitle>
          <CardDescription>View logs for any hosted process</CardDescription>
        </CardHeader>
        <CardContent>
          <SiteLogs noTopMargin/>
        </CardContent>
      </Card>
    </main>
  );
};

export default LogsPage;
