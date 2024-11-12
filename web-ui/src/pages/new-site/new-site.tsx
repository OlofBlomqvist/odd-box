import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NewRemoteSiteSettings from "./new-remote-site-settings";

const NewSitePage = () => {
  return (
    <main className="grid flex-1 items-start gap-4 md:gap-8 max-w-[900px]">
      <Card>
        <CardHeader>
          <CardTitle>New remote site</CardTitle>
          <CardDescription>
            A remote site forwards traffic to external servers.
            <br/>
            You can add more backends to a site after creating it.
            </CardDescription>
        </CardHeader>
        <CardContent>
          <NewRemoteSiteSettings />
        </CardContent>
      </Card>
    </main>
  );
};

export default NewSitePage;
