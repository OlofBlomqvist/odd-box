import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import toast from "react-hot-toast";
import { useEffect, useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { Backend, RemoteSiteConfig } from "../../generated-api";
import SettingDescriptions from "@/lib/setting_descriptions";
import { BackendSheet } from "@/components/sheet/backend_sheet/backend_sheet";
import { BackendsTable } from "@/components/table/backends/backends";
import { ConfirmationDialog } from "@/components/dialog/confirm/confirm";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

type BackendModalState = {
  show: boolean;
  backend: Backend | undefined;
  listIndex: number;
};

const RemoteSiteSettings = ({ site }: { site: RemoteSiteConfig }) => {
  const { deleteSite, updateRemoteSite } = useSiteMutations();
  const [newName, setNewName] = useState(site?.host_name);

  const [showConfirmDeleteModal, setShowConfirmDeleteModal] = useState(false);
  const [backendModalState, setBackendModalState] = useState<BackendModalState>(
    {
      backend: undefined,
      show: false,
      listIndex: -1,
    }
  );

  useEffect(() => {
    setBackendModalState((old) => ({
      ...old,
      backends: site.backends,
    }));
  }, [site]);
  const router = useRouter();

  const updateSetting = (key: string, value: any) => {
    let val =
      Array.isArray(value) || isNaN(value) === false ? value : `${value}`;

    toast.promise(
      updateRemoteSite.mutateAsync({
        hostname: site.host_name,
        siteSettings: {
          ...site,
          [key]: val,
        },
      }),
      {
        loading: `Updating settings.. [${site.host_name}]`,
        success: `Settings updated! [${site.host_name}]`,
        error: (e) => `${e}`,
      }
    );
  };

  return (

<main
      className="grid flex-1 items-start gap-4 sm:py-0 md:gap-8 max-w-[900px]"
      key={site.host_name}
      style={{}}
      onSubmit={(e) => {
        e.preventDefault();
      }}
    >
      <Card className="mb-8">
        
      <CardHeader>
            <CardTitle>Site details</CardTitle>
            <CardDescription>
              General configuration for{" "}
              <span className="font-bold text-[var(--accent-text)]">{site.host_name}</span>
            </CardDescription>
          </CardHeader>
        <CardContent>
        <SettingsSection marginTop="0px" noTopSeparator>
        <SettingsItem
          title="Hostname"
          subTitle={SettingDescriptions["hostname_frontend"]}
        >
          <Input
            originalValue={site.host_name}
            onSave={(newValue) => {
              updateSetting("host_name", newValue);
            }}
            withSaveButton
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
        </SettingsItem>
      </SettingsSection>

      <SettingsSection noTopSeparator>
        <SettingsItem
          labelFor="capture_subdomains"
          rowOnly
          title="Capture sub-domains"
          subTitle={SettingDescriptions["capture_subdomains"]}
        >
          <Input
            onChange={() => {
              updateSetting("capture_subdomains", !site.capture_subdomains);
            }}
            checked={Boolean(site.capture_subdomains)}
            type="checkbox"
            name="capture_subdomains"
            id="capture_subdomains"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>

        <SettingsItem
          rowOnly
          labelFor="disable_tcp_tunnel"
          title="Disable TCP tunnel mode"
          subTitle={SettingDescriptions["disable_tcp_tunnel"]}
        >
          <Input
            type="checkbox"
            checked={Boolean(site.disable_tcp_tunnel_mode)}
            onChange={() => {
              updateSetting(
                "disable_tcp_tunnel_mode",
                !site.disable_tcp_tunnel_mode
              );
            }}
            id="disable_tcp_tunnel"
            name="disable_tcp_tunnel"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>

        <SettingsItem
          rowOnly
          labelFor="forward_subdomains"
          title="Forward sub-domains"
          subTitle={SettingDescriptions["forward_subdomains"]}
        >
          <Input
            type="checkbox"
            checked={Boolean(site.forward_subdomains)}
            onChange={() => {
              updateSetting("forward_subdomains", !site.forward_subdomains);
            }}
            id="forward_subdomains"
            name="forward_subdomains"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>
      </SettingsSection>
      <SettingsSection noTopSeparator noBottomSeparator>
        <SettingsItem
          title="Backends"
          subTitle={SettingDescriptions["backends"]}
        />
        <BackendsTable site={site} />
      </SettingsSection>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "end",
          marginTop: "20px",
        }}
      >
        <Button
          onClick={() => {
            setShowConfirmDeleteModal(true);
          }}
          style={{ width: "max-content" }}
          dangerButton
        >
          Delete site
        </Button>
      </div>
      <ConfirmationDialog
      isDangerAction
        onClose={() => setShowConfirmDeleteModal(false)}
        onConfirm={() => {
          setShowConfirmDeleteModal(false);
          deleteSite.mutateAsync(
            { hostname: site.host_name },
            {
              onSuccess: () => {
                setShowConfirmDeleteModal(false);
                router.navigate({ to: "/", search: { type: "processes" } });
              },
            }
          );
        }}
        show={showConfirmDeleteModal}
        title="Delete"
        yesBtnText="Yes, delete it"
        subtitle={<span>Are you sure you want to delete <span className="font-bold text-[var(--accent-text)]">{site.host_name}</span>?</span>}
        />

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
        </CardContent>
        </Card>
        </main>


  );
};

export default RemoteSiteSettings;
