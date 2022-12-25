import Head from "next/head";
import styles from "../styles/Home.module.css";
import CircularProgress from "@mui/material/CircularProgress";
import { open } from "@tauri-apps/api/dialog";
import { emit, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/tauri";

import { useEffect, useState } from "react";

export default function Home() {
  const [audioMessage, setAudioMessage] = useState("");
  const [progress, setProgress] = useState(false);

  useEffect(() => {
    const handler = async () => {
      interface Payload {
        message: string;
        i: [[number, number]];
      }
      const unlisten = await listen<Payload>("event-name", (event) => {
        setProgress(false);
        setAudioMessage(event.payload.message);
      });
    };
    handler().catch(console.error);
  }, []);

  const selectMp4 = async () => {
    // Open a selection dialog for image files
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Video",
          extensions: ["mp4"],
        },
      ],
    });
    if (Array.isArray(selected)) {
      // user selected multiple files
      setProgress(true);
      const s = await invoke<string>("greet", { path: selected[0] });
    } else if (selected === null) {
      // user cancelled the selection
    } else {
      // user selected a single file
    }
  };

  return (
    <div className={styles.container}>
      <Head>
        <title>Create Next App</title>
        <meta name="description" content="Generated by create next app" />
        <link rel="icon" href="/favicon.ico" />
      </Head>

      <main className={styles.main}>
        <h5 className={styles.text}>{audioMessage}</h5>
        <h5 className={styles.text}>{progress}</h5>
        {progress && <CircularProgress />}
        <button className={styles.card} onClick={selectMp4}>
          Select MP4
        </button>
      </main>

      <footer className={styles.footer}>
        <a href="miantu.net" target="_blank" rel="noopener noreferrer">
          Powered by GoooIce
        </a>
      </footer>
    </div>
  );
}
