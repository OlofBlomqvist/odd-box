import { EnvVarsSheet } from "@/components/sheet/env_vars_sheet/env_vars_sheet";
import {
  Table,
  TableBody,
  TableCell,
  TableFooter,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/table/table";
import { KvP } from "@/generated-api";
import { DiamondPlus } from "lucide-react";
import { useState } from "react";

export function EnvVariablesTable({
  keys,
  onNewKey,
  onRemoveKey,
}: {
  onRemoveKey?: (keyName: string) => void;
  onNewKey?: (newKey: KvP, originalName: string | undefined) => void;
  keys: Array<KvP>;
}) {
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

  const onCreateNewVariable = (
    newKey: KvP,
    originalName: string | undefined
  ) => {
    onNewKey?.(newKey, originalName);
    setModalState((old) => ({
      ...old,
      name: newKey.key,
      originalName: newKey.key,
      originalValue: "",
      value: "",
    }));
  };

  const footerClassNames = ["hover:cursor-pointer"];

  if (keys.length === 0) {
    footerClassNames.push("border-0");
  }

  return (
    <>
      <Table>
        {keys.length !== 0 && (<TableHeader>
          <TableRow className="pointer-events-none">
            <TableHead className="w-[100px] text-[var(--color2)]">
              Name
            </TableHead>
            <TableHead className="text-[var(--color2)]">Value</TableHead>
          </TableRow>
        </TableHeader>)}
        <TableBody>
          {keys.map((kvp) => (
            <TableRow
              key={JSON.stringify(kvp)}
              className="hover:cursor-pointer"
              onClick={() => {
                setModalState({
                  show: true,
                  name: kvp.key,
                  value: kvp.value,
                  originalName: kvp.key,
                  originalValue: kvp.value,
                });
              }}
            >
              <TableCell className="font-medium">{kvp.key}</TableCell>
              <TableCell>{kvp.value}</TableCell>
            </TableRow>
          ))}
        </TableBody>
        <TableFooter className={footerClassNames.join(" ")}>
          <TableRow onClick={newVariableClicked}>
            <TableCell className="bg-transparent" colSpan={3}>
              <div className="flex items-center gap-2 justify-center">
                <DiamondPlus />
                <span>Add new environment variable</span>
              </div>
            </TableCell>
          </TableRow>
        </TableFooter>
      </Table>
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
}
