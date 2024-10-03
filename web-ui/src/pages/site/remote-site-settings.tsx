import SettingsItem from "../settings/settings-item";
import SettingsSection from "../settings/settings-section";
import Input from "../../components/input/input";
import "./style.css";
import Button from "../../components/button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import toast from "react-hot-toast";
import { useEffect, useState } from "react";
import { useRouter } from "@tanstack/react-router";
import { Backend, RemoteSiteConfig } from "../../generated-api";
import Plus2 from "../../components/icons/plus2";
import SettingDescriptions from "@/lib/setting_descriptions";
import { BackendSheet } from "@/components/sheet/backend_sheet/backend_sheet";
import OddModal from "@/components/modal/modal";

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
    <div
      key={site.host_name}
      style={{ paddingBottom: "50px", maxWidth: "750px" }}
      onSubmit={(e) => {
        e.preventDefault();
      }}
    >
      <SettingsSection noTopSeparator>
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
      <SettingsSection noTopSeparator>
        <SettingsItem title="Backends" subTitle={SettingDescriptions["backends"]} />
        <div
          style={{
            background: "var(--color3)",
            color: "black",
            marginTop: "10px",
            borderRadius: "5px",
            overflow: "hidden",
          }}
        >
          {site.backends?.map((key, listIndex) => (
            <div
              key={JSON.stringify({ backend: key, index: listIndex })}
              onClick={() => {
                setBackendModalState({
                  backend: key,
                  show: true,
                  listIndex,
                });
              }}
              className="env-var-item"
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                padding: "5px",
              }}
            >
              <p style={{ zIndex: 1, fontSize: ".8rem" }}>{key.address}</p>
            </div>
          ))}
          <div
            onClick={() => {
              updateRemoteSite.mutateAsync({
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
            }}
            className="env-var-item"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "5px",
            }}
          >
            <div
              style={{
                zIndex: 1,
                fontSize: ".8rem",
                display: "flex",
                alignItems: "center",
                gap: "5px",
              }}
            >
              <Plus2 />
              New backend
            </div>
          </div>
        </div>
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
      <OddModal
        key={site.host_name}
        show={showConfirmDeleteModal}
        onClose={() => setShowConfirmDeleteModal(false)}
        title="Delete"
        subtitle={`Are you sure you want to delete the site '${site.host_name}'?`}
      >
        <div style={{ display: "flex", gap: "20px", marginTop: "10px" }}>
          <Button secondary onClick={() => setShowConfirmDeleteModal(false)}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              deleteSite.mutateAsync(
                { hostname: site.host_name },
                {
                  onSuccess: () => {
                    setShowConfirmDeleteModal(false);
                    router.navigate({ to: "/" });
                  },
                }
              );
            }}
            dangerButton
          >
            Yes, delete
          </Button>
        </div>
      </OddModal>
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
    </div>
  );
};

export default RemoteSiteSettings;
