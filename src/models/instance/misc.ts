import { ExportModpackFormat, ModLoaderType } from "@/enums/instance";
import { OtherResourceSource } from "@/enums/resource";

export enum ModLoaderStatus {
  NotDownloaded = "NotDownloaded",
  Downloading = "Downloading",
  DownloadFailed = "DownloadFailed",
  Installing = "Installing",
  Installed = "Installed",
}

export interface ModLoader {
  status: ModLoaderStatus;
  loaderType: ModLoaderType;
  version?: string;
  branch?: string;
}

export interface OptiFine {
  filename: string;
  version: string;
  status: ModLoaderStatus;
}

export interface InstanceSummary {
  id: string;
  iconSrc: string;
  name: string;
  description?: string;
  tag?: string;
  starred: boolean;
  playTime: number;
  versionPath: string;
  version: string;
  majorVersion: string;
  modLoader: ModLoader;
  optifine?: OptiFine;
  supportQuickPlay: boolean;
  useSpecGameConfig: boolean;
  isVersionIsolated: boolean;
  modpackVersion?: string;
}

export interface ModpackMetaInfo {
  name: string;
  version?: string;
  author?: string;
  description?: string;
  modpackType: OtherResourceSource;
  clientVersion: string;
  modLoader?: ModLoader;
}

export interface ModpackFileList {
  all: string[];
  unchecked: string[];
}

export interface ExportModpackOptions {
  format: ExportModpackFormat;
  name: string;
  version: string;
  author?: string;
  description?: string;
  packWithLauncher?: boolean;
  minMemory?: number;
  noCreateRemoteFiles?: boolean;
  skipCurseForgeRemoteFiles?: boolean;
}

export interface GameServerInfo {
  iconSrc: string;
  ip: string;
  name: string;
  description: string;
  isQueried: boolean;
  playersOnline?: number;
  playersMax?: number;
  online: boolean;
  latency?: number;
}

export interface LocalModInfo {
  iconSrc: string;
  enabled: boolean;
  modId: string;
  name: string;
  translatedName?: string;
  version: string;
  loaderType: ModLoaderType;
  fileName: string;
  filePath: string;
  relativePath: string;
  description?: string;
  translatedDescription?: string;
  potentialIncompatibility: boolean;
}

export interface ResourcePackInfo {
  name: string;
  description?: string;
  iconSrc?: string;
  filePath: string;
}

export interface SchematicInfo {
  name: string;
  filePath: string;
  relativePath: string;
}

export interface ShaderPackInfo {
  fileName: string;
  filePath: string;
}

export interface ScreenshotInfo {
  fileName: string;
  filePath: string;
  time: number; // UNIX timestamp
}

export interface WeeklyPlayTimeDay {
  date: string;
  seconds: number;
}

export interface WeeklyPlayTime {
  days: WeeklyPlayTimeDay[];
  weekSeconds: number;
  todaySeconds: number;
}

/** Play time windows selectable on the launch page. */
export const PLAY_TIME_WINDOWS = [7, 14, 30] as const;

export type PlayTimeWindow = (typeof PLAY_TIME_WINDOWS)[number];

/** The window used when the user has not picked one. */
export const DEFAULT_PLAY_TIME_WINDOW: PlayTimeWindow = 14;
