import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NewHostedProcessSettings from "./new-hosted-process-settings";

const NewProcessPage = () => {
  return (
    <main className="grid flex-1 items-start gap-4 md:pb-8 md:gap-8 max-w-[900px]">
      <Card>
        <CardHeader>
          <CardTitle>New hosted process</CardTitle>
          <CardDescription>
            Creating a process that odd-box will manage.
            <br/>
            This is a service that odd-box can start, stop, and restart.
            </CardDescription>
        </CardHeader>
        <CardContent>
          <NewHostedProcessSettings />
        </CardContent>
      </Card>
    </main>
  );
};

export default NewProcessPage;
