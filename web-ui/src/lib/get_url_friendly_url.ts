export const getUrlFriendlyUrl = (url: string) => {
  return url.replaceAll("http://", "").replaceAll("https://", "");
}
