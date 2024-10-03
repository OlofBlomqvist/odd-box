import Button from "../button/button";
import { useState } from "react";
import Plus2 from "../icons/plus2";
import Input from "../input/input";
import "./styles.css";
import "react-responsive-modal/styles.css";
import { KvP } from "../../generated-api";
import OddModal from "../modal/modal";

const KeyValueInput = ({
  keys,
  onNewKey,
  onRemoveKey,
}: {
  onRemoveKey?: (keyName: string) => void;
  onNewKey?: (newKey: KvP, originalName: string | undefined) => void;
  keys: Array<KvP>;
}) => {
  const [modalState, setModalState] = useState<{
    show: boolean;
    name: string;
    value: string;
    originalName: string | undefined;
  }>({
    show: false,
    name: "",
    value: "",
    originalName: undefined,
  });
  const newVariableClicked = () => {
    setModalState((x) => ({
      show: !x.show,
      name: "",
      value: "",
      originalName: undefined,
    }));
  };

  return (
    <>
      <div
        style={{
          background: "var(--color3)",
          color: "black",
          marginTop: "10px",
          borderRadius: "5px",
          overflow: "hidden",
        }}
      >
        {keys.map((key) => (
          <div
            key={key.key}
            onClick={() => {
              setModalState({
                show: true,
                name: key.key,
                value: key.value,
                originalName: key.key,
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
            <p style={{ zIndex: 1, fontSize: ".8rem" }}>{key.key}</p>
            <p style={{ zIndex: 1, fontSize: ".8rem" }}>{key.value}</p>
          </div>
        ))}
        <div
          onClick={newVariableClicked}
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
            Add new variable
          </div>
        </div>
      </div>
      <OddModal
        show={modalState.show}
        onClose={() => setModalState((old) => ({ ...old, show: false }))}
        title={modalState.originalName ? "Edit variable" : "New variable"}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div>
            <p style={{ fontSize: ".8rem" }}>NAME</p>
            <Input
              type="text"
              value={modalState.name}
              onChange={(e) =>
                setModalState((old) => ({ ...old, name: e.target.value }))
              }
            />
          </div>
          <div>
            <p style={{ fontSize: ".8rem" }}>VALUE</p>
            <Input
              type="text"
              value={modalState.value}
              onChange={(e) =>
                setModalState((old) => ({ ...old, value: e.target.value }))
              }
            />
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: "10px",
              marginTop: "5px",
            }}
          >
            {modalState.originalName !== undefined && (
              <Button
                dangerButton
                onClick={() => {
                  onRemoveKey?.(modalState.originalName!);
                  setModalState((old) => ({ ...old, show: false }));
                }}
              >
                Delete
              </Button>
            )}
            <Button
              secondary
              dangerButton
              onClick={() => setModalState((old) => ({ ...old, show: false }))}
            >
              Cancel
            </Button>
            <Button
              disabled={modalState.name === "" || modalState.value === ""}
              onClick={() => {
                onNewKey?.(
                  {
                    key: modalState.name,
                    value: modalState.value,
                  },
                  modalState.originalName
                );
                setModalState((old) => ({ ...old, show: false }));
              }}
            >
              Save
            </Button>
          </div>
        </div>
      </OddModal>
    </>
  );
};

export default KeyValueInput;
