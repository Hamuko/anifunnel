import "./Footer.css";

function AppBuild() {
  if (import.meta.env.VITE_APP_BUILD) {
    return <>({import.meta.env.VITE_APP_BUILD})</>;
  }
  return null;
}

function AppVersion() {
  if (import.meta.env.VITE_APP_VERSION) {
    return <>v{import.meta.env.VITE_APP_VERSION}</>;
  }
  return null;
}

function Footer() {
  return (
    <footer>
      anifunnel <AppVersion /> <AppBuild />
    </footer>
  );
}

export default Footer;
