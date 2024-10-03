import { BackendSheet } from "@/components/sheet/backend_sheet/backend_sheet";
import {
  Table,
  TableBody,
  TableCell,
  TableFooter,
  TableRow,
} from "@/components/table/table";
import { Backend, RemoteSiteConfig } from "@/generated-api";
import useSiteMutations from "@/hooks/use-site-mutations";
import { DiamondPlus } from "lucide-react";
import { useState } from "react";

type BackendModalState = {
    show: boolean;
    backend: Backend | undefined;
    listIndex: number;
  };

export const BackendsTable = ({
  site,
}: {
  site:RemoteSiteConfig
}) => {
    const { updateRemoteSite } = useSiteMutations();

    const [backendModalState, setBackendModalState] = useState<BackendModalState>(
        {
          backend: undefined,
          show: false,
          listIndex: -1,
        }
      );
  const newBackendClicked = async () => {
    await updateRemoteSite.mutateAsync({
        hostname: site.host_name,
        siteSettings: {
          ...site,
          backends: [
            ...site.backends,
            {
              address: "NEW_BACKEND",
              port: 8080,
              hints: [],
              https: false,
            },
          ],
        },
      });
  };

  const footerClassNames = ["hover:cursor-pointer"];

  if (site.backends?.length === 0) {
    footerClassNames.push("border-0");
  }

  return (
    <>
      <Table>
        <TableBody>
          {site.backends?.map((backend,listIndex) => (
            <TableRow
              key={JSON.stringify(backend)}
              className="hover:cursor-pointer"
              onClick={() => {
                setBackendModalState({
                    backend,
                    show: true,
                    listIndex,
                  });
              }}
            >
              <TableCell className="font-medium">{backend.address}</TableCell>
            </TableRow>
          ))}
        </TableBody>
        <TableFooter className={footerClassNames.join(" ")}>
          <TableRow onClick={newBackendClicked}>
            <TableCell className="bg-transparent" colSpan={3}>
              <div className="flex items-center gap-2 justify-center">
                <DiamondPlus />
                <span>Add new backend</span>
              </div>
            </TableCell>
          </TableRow>
        </TableFooter>
      </Table>
      <BackendSheet
        listIndex={backendModalState.listIndex}
        key={JSON.stringify(backendModalState.backend)}
        site={site}
        show={backendModalState.show}
        onClose={() =>
          setBackendModalState((old) => ({
            ...old,
            show: false,
          }))
        }
      />
    </>
  );
};
