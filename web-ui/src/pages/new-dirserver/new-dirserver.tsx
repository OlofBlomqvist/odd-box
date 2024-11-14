import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NewDirServerSettings from "./new-dirserver-settings";

const NewDirServerPage = () => {
  return (
    <main className="grid flex-1 items-start gap-4 md:gap-8 max-w-[900px]">
      <Card>
        <CardHeader>
          <CardTitle>New static site</CardTitle>
          <CardDescription>
            A static site configuration allows you to serve files from a
            directory on the local filesystem.
            <br />
            Both unencrypted (http) and encrypted (https) connections are
            supported, either self-signed or thru lets-encrypt.
            <br />
            You can specify rules for how the cache should behave, and you can
            also specify rules for how the files should be served.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <NewDirServerSettings />
        </CardContent>
      </Card>
    </main>
  );
};

export default NewDirServerPage;
