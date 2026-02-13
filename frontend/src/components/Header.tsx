import "./Header.css";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { User } from "../types";

function ExpiryText({ expiry }: { expiry: number }) {
  function dayUntilExpiry(date: Date): number {
    const diff = date.valueOf() - Date.now().valueOf();
    return Math.floor(diff / 86400000);
  }

  const daysUntilExpiry = dayUntilExpiry(new Date(expiry * 1000));

  if (daysUntilExpiry > 30) {
    const months = Math.round(daysUntilExpiry / 30.4166);
    return <>{months} months</>;
  }

  return <span className="warn">{daysUntilExpiry} days</span>;
}

function LogoutButton() {
  const queryClient = useQueryClient();

  const { mutate } = useMutation({
    mutationFn: async () => {
      const response = await fetch("/api/user", {
        method: "DELETE",
      });
      if (!response.ok) {
        const data = await response.json();
        throw Error(data.error);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user"] });
    },
  });

  return <a onClick={() => mutate()}>Log out.</a>;
}

function UserInfo({ user }: { user: User | null }) {
  return (
    <div className="user-info">
      <h4>{user ? user.name : "No Anilist token set"}</h4>

      {user ? (
        <>
          <h5>
            Token will expire in approximately{" "}
            <ExpiryText expiry={user.expiry} />. <LogoutButton />
          </h5>
        </>
      ) : (
        <h5>Token must be set in order to use anifunnel</h5>
      )}
    </div>
  );
}

function Header({ user }: { user: User | null }) {
  return (
    <header>
      <div className="wrapper">
        <h1>anifunnel</h1>
        <UserInfo user={user} />
      </div>
    </header>
  );
}

export default Header;
