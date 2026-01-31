import { useQuery } from "@tanstack/react-query";
import "./App.css";
import AuthenticationForm from "./components/Authentication";
import Header from "./components/Header";
import Footer from "./components/Footer";
import AnimeList from "./components/AnimeList";
import type { User } from "./types";

function MainContent({
  user,
  loading,
  error,
}: {
  user: User | null;
  loading: boolean;
  error: Error | null;
}) {
  if (loading) {
    return <p>Loading data...</p>;
  }

  if (error) {
    return <p>Could not load data: {error.message}</p>;
  }

  return <>{user ? <AnimeList /> : <AuthenticationForm />}</>;
}

function App() {
  const userQuery = useQuery({
    queryKey: ["user"],
    queryFn: async () => {
      const response = await fetch("/api/user");
      return await response.json();
    },
    staleTime: 5 * 60 * 1000,
  });

  return (
    <>
      <Header user={userQuery.data} />
      <main>
        <div className="wrapper">
          <MainContent
            user={userQuery.data}
            loading={userQuery.isPending}
            error={userQuery.error}
          />
        </div>
      </main>
      <Footer />
    </>
  );
}

export default App;
