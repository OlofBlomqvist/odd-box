import { useState } from "react";
import Plus2 from "../icons/plus2";
import "./styles.css";
import "react-responsive-modal/styles.css";
import { KvP } from "../../generated-api";
import { EnvVarsSheet } from "../sheet/env_vars_sheet/env_vars_sheet";

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
    originalValue: string | undefined;
  }>({
    show: false,
    name: "",
    value: "",
    originalName: undefined,
    originalValue: undefined,
  });
  const newVariableClicked = () => {
    setModalState((x) => ({
      show: !x.show,
      name: "",
      value: "",
      originalName: "",
      originalValue: "",
    }));
  };

  const onCreateNewVariable = (newKey:KvP, originalName:string|undefined) => {
    onNewKey?.(newKey, originalName);
    setModalState(old => ({
      ...old,
      name: newKey.key,
      originalName: newKey.key,
      originalValue: "",
      value: ""
    }))
  }

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
                originalValue: key.value,
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

      <EnvVarsSheet
        onNewKey={onCreateNewVariable}
        onRemoveKey={onRemoveKey}
        onClose={() => setModalState((old) => ({ ...old, show: false }))}
        name={modalState.name}
        originalValue={modalState.originalValue}
        show={modalState.show}
        value={modalState.value}
        valueChanged={(e) => setModalState((old) => ({ ...old, value: e }))}
      />
    </>
  );
};

export default KeyValueInput;
