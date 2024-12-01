export function envVarsToString(arr: Array<{ key: string; value: string }>) {
  return arr.map((obj) => `${obj.key}=${obj.value}`).join(";");
}

export function envVarsStringToArray(str: string) {
  return str === "" ? undefined : str.split(";").map((pair) => {
    const [key, value] = pair.split("=");
    return { key, value };
  });
}
