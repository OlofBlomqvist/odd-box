import { useState } from "react";
import { Backend, Hint, RemoteSiteConfig } from "../../generated-api";
import { useRemoteSites } from "../../hooks/use-remote-sites";
import SettingsItem from "../../pages/settings/settings-item";
import SettingsSection from "../../pages/settings/settings-section";
import Input from "../input/input";
import OddModal from "../modal/modal";
import Button from "../button/button";
import useSiteMutations from "../../hooks/use-site-mutations";
import Checkbox from "../checkbox/checkbox";
import SettingDescriptions from "@/lib/setting_descriptions";

const BackendModal = ({
  site,
  show,
  onClose,
  listIndex,
}: {
  listIndex: number;
  onClose: () => void;
  show: boolean;
  site: RemoteSiteConfig;
}) => {
  const { data: sites } = useRemoteSites();
  const { updateRemoteSite } = useSiteMutations();

  const thisSite = sites.find((x) => x.host_name === site.host_name);
  const thisBackend = thisSite?.backends[listIndex];

  const [newAddress, setNewAddress] = useState<string>(
    thisBackend?.address ?? ""
  );
  const [newPort, setNewPort] = useState<number>(thisBackend?.port ?? 8080);
  const useHttps = thisBackend?.https ?? false;

  const saveSettings = ({
    newBackendSettings,
  }: {
    newBackendSettings: Backend;
  }) => {
    const newBackends = [
      ...site.backends.slice(0, listIndex),
      newBackendSettings,
      ...site.backends.slice(listIndex + 1),
    ];

    updateRemoteSite.mutateAsync({
      hostname: site.host_name,
      siteSettings: {
        ...site,
        backends: newBackends,
      },
    });
  };

  if (!thisBackend) {
    return null;
  }

  return (
    <OddModal
      show={show}
      onClose={onClose}
      subtitle={thisBackend?.address}
      title={`Edit backend`}
    >
      <SettingsSection>
        <SettingsItem
          title="Address"
          subTitle={SettingDescriptions["hostname_frontend"]}
        >
          <Input
            originalValue={thisBackend?.address}
            onSave={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: newAddress,
                  hints: thisBackend?.hints ?? [],
                  https: useHttps,
                },
              });
            }}
            withSaveButton
            value={newAddress}
            onChange={(e) => {
              setNewAddress(e.target.value);
            }}
          />
        </SettingsItem>
        <SettingsItem title="Port" subTitle={SettingDescriptions["port"]}>
          <Input
            originalValue={thisBackend?.port}
            withSaveButton
            onSave={() => {
              saveSettings({
                newBackendSettings: {
                  port: newPort,
                  address: thisBackend.address,
                  hints: thisBackend?.hints ?? [],
                  https: useHttps,
                },
              });
            }}
            value={newPort}
            onChange={(e) => {
              if (isNaN(Number(e.target.value))) {
                return;
              }
              setNewPort(Number(e.target.value));
            }}
          />
        </SettingsItem>
        <SettingsItem
          labelFor="use_https"
          rowOnly
          title="HTTPS"
          subTitle={SettingDescriptions["https"]}
        >
          <Input
            checked={useHttps}
            onChange={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: thisBackend.address,
                  hints: thisBackend?.hints ?? [],
                  https: !useHttps,
                },
              });
            }}
            name="use_https"
            id="use_https"
            type="checkbox"
            style={{ width: "20px", height: "20px" }}
          />
        </SettingsItem>

        <SettingsItem title="Hints" subTitle={SettingDescriptions["h2_hint"]}></SettingsItem>
        <div
          style={{
            display: "flex",
            gap: "10px",
            flexWrap: "wrap",
            justifyContent: "start",
          }}
        >
          <Checkbox
            onClick={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: thisBackend.address,
                  hints: thisBackend?.hints?.includes(Hint.H2)
                    ? thisBackend?.hints.filter((x) => x !== Hint.H2)
                    : [...(thisBackend?.hints ?? []), Hint.H2],
                  https: useHttps,
                },
              });
            }}
            checked={Boolean(thisBackend?.hints?.includes(Hint.H2))}
            title="H2"
          />
          <Checkbox
            onClick={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: thisBackend.address,
                  hints: thisBackend?.hints?.includes(Hint.H2C)
                    ? thisBackend?.hints.filter((x) => x !== Hint.H2C)
                    : [...(thisBackend?.hints ?? []), Hint.H2C],
                  https: useHttps,
                },
              });
            }}
            checked={Boolean(thisBackend?.hints?.includes(Hint.H2C))}
            title="H2C"
          />
          <Checkbox
            onClick={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: thisBackend.address,
                  hints: thisBackend?.hints?.includes(Hint.H2CPK)
                    ? thisBackend?.hints.filter((x) => x !== Hint.H2CPK)
                    : [...(thisBackend?.hints ?? []), Hint.H2CPK],
                  https: useHttps,
                },
              });
            }}
            checked={Boolean(thisBackend?.hints?.includes(Hint.H2CPK))}
            title="H2CPK"
          />
          <Checkbox
            onClick={() => {
              saveSettings({
                newBackendSettings: {
                  port: thisBackend.port,
                  address: thisBackend.address,
                  hints: thisBackend?.hints?.includes(Hint.NOH2)
                    ? thisBackend?.hints.filter((x) => x !== Hint.NOH2)
                    : [...(thisBackend?.hints ?? []), Hint.NOH2],
                  https: useHttps,
                },
              });
            }}
            checked={Boolean(thisBackend?.hints?.includes(Hint.NOH2))}
            title="NOH2"
          />
        </div>
      </SettingsSection>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: ".5fr 1fr",
          gap: "10px",
          marginTop: "5px",
        }}
      >
        <Button
          onClick={async () => {
            try {
              await updateRemoteSite.mutateAsync({
                hostname: site.host_name,
                siteSettings: {
                  ...site,
                  backends: site.backends.filter((_x, i) => i !== listIndex),
                },
              });
              onClose();
            } catch (e) {
              console.error("delete backend error:", e);
            }
          }}
          style={{}}
          dangerButton
        >
          Delete backend
        </Button>
        <Button style={{}} onClick={onClose}>
          Close
        </Button>
      </div>
    </OddModal>
  );
};

export default BackendModal;
