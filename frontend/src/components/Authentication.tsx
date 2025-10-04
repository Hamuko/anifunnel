import "./Authentication.css";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { Authentication } from "../types";

function AuthenticationButton() {
  const anilistAuthURL =
    "https://anilist.co/api/v2/oauth/authorize?client_id=9878&response_type=token";

  function startAuthentication() {
    window.open(
      anilistAuthURL,
      "auth",
      "scrollbars=no,resizable=no,status=no,location=no,toolbar=no,menubar=no,width=800,height=500",
    );
  }

  return <button onClick={startAuthentication}>Get token</button>;
}

export default function AuthenticationForm() {
  const queryClient = useQueryClient();

  const { mutate, error, isPending } = useMutation({
    mutationFn: async (authentication: Authentication) => {
      const response = await fetch("/api/user", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(authentication),
      });
      if (!response.ok) {
        const data = await response.json();
        throw Error(data.error);
      }
      return null;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user"] });
    },
  });

  function authenticate(formData: FormData) {
    const token = formData.get("token");
    if (typeof token === "string") {
      mutate({ token: token });
    }
  }

  return (
    <form id="authentication-form" action={authenticate}>
      <h2>Authenticate with Anilist</h2>
      <div className="container">
        <textarea name="token" rows={5} placeholder="Anilist token" required />
      </div>
      {error && <p className="error-text">{error.message}</p>}
      <div className="buttons">
        <AuthenticationButton />
        <button type="submit" disabled={isPending}>
          Authenticate
        </button>
      </div>
    </form>
  );
}
