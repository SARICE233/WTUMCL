import {
  Center,
  HStack,
  Image,
  Radio,
  RadioGroup,
  Tag,
  VStack,
} from "@chakra-ui/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { BeatLoader } from "react-spinners";
import Empty from "@/components/common/empty";
import {
  OptionItemProps,
  VirtualOptionItemGroup,
} from "@/components/common/option-item-virtual";
import { Section } from "@/components/common/section";
import SelectableCard, {
  SelectableCardProps,
} from "@/components/common/selectable-card";
import { useLauncherConfig } from "@/contexts/config";
import { useToast } from "@/contexts/toast";
import { ModLoaderType } from "@/enums/instance";
import {
  GameClientResourceInfo,
  ModLoaderResourceInfo,
  OptiFineResourceInfo,
  defaultModLoaderResourceInfo,
} from "@/models/resource";
import { ResourceService } from "@/services/resource";
import { ISOToDatetime } from "@/utils/datetime";

export const modLoaderTypes: ModLoaderType[] = [
  ModLoaderType.Forge,
  ModLoaderType.Cleanroom,
  ModLoaderType.Fabric,
  ModLoaderType.NeoForge,
  ModLoaderType.Quilt,
];

export const modLoaderTypesToIcon: Record<string, string> = {
  Unknown: "",
  Fabric: "Fabric.png",
  Forge: "Forge.png",
  Cleanroom: "Cleanroom.png",
  NeoForge: "NeoForge.png",
  Quilt: "Quilt.png",
};

interface LoaderSelectorProps {
  selectedGameVersion: GameClientResourceInfo;
  selectedModLoader: ModLoaderResourceInfo;
  onSelectModLoader: (v: ModLoaderResourceInfo) => void;
  selectedOptiFine?: OptiFineResourceInfo | undefined;
  onSelectOptiFine?: (v: OptiFineResourceInfo | undefined) => void;
  mode?: "all" | "optifine";
}

export const LoaderSelector: React.FC<LoaderSelectorProps> = ({
  selectedGameVersion,
  selectedModLoader,
  onSelectModLoader,
  selectedOptiFine,
  onSelectOptiFine,
  mode = "all",
  ...props
}) => {
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const toast = useToast();
  const primaryColor = config.appearance.theme.primaryColor;
  const [versionList, setVersionList] = useState<OptionItemProps[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedType, setSelectedType] = useState<ModLoaderType | "OptiFine">(
    ModLoaderType.Unknown
  );
  const [selectedId, setSelectedId] = useState("");

  const selectableCardListRef = useRef<HTMLDivElement>(null);
  // Token of the version list request whose response is still wanted: bumping
  // it when the loader type changes makes any late answer for the previous
  // type a no-op, and the AbortController tears the in-flight one down.
  const requestIdRef = useRef(0);
  const requestAbortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (mode === "optifine") {
      setSelectedType("OptiFine");
      setSelectedId(selectedOptiFine?.filename || "");
      return;
    }
    const externalType = selectedModLoader.loaderType;
    if (
      externalType !== ModLoaderType.Unknown &&
      selectedType !== externalType
    ) {
      setSelectedType(externalType);

      if (selectedModLoader.version) {
        setSelectedId(selectedModLoader.version);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, selectedModLoader.loaderType]);

  function isModLoaderResourceInfo(
    version: ModLoaderResourceInfo | OptiFineResourceInfo
  ): version is ModLoaderResourceInfo {
    return (version as ModLoaderResourceInfo).loaderType !== undefined;
  }

  const buildOptionItems = useCallback(
    (
      version: ModLoaderResourceInfo | OptiFineResourceInfo
    ): OptionItemProps => {
      const isModLoader = isModLoaderResourceInfo(version);
      let title = isModLoader ? version.version : version?.filename;
      return {
        title,
        description: isModLoader && version.description,
        prefixElement: (
          <HStack spacing={2.5}>
            <Radio value={title} colorScheme={primaryColor} />
            <Image
              src={`/images/icons/${isModLoader ? modLoaderTypesToIcon[version.loaderType] : "OptiFine.png"}`}
              alt={title}
              boxSize="28px"
              borderRadius="4px"
            />
          </HStack>
        ),
        titleExtra: (() => {
          const stableFlag = isModLoader
            ? version.stable
            : !version.patch.startsWith("pre");
          if (stableFlag == null) return;
          return (
            <Tag colorScheme={primaryColor} className="tag-xs">
              {t(`LoaderSelector.${stableFlag ? "stable" : "beta"}`)}
            </Tag>
          );
        })(),
        children: <></>,
        isFullClickZone: true,
        onClick: () => {
          if (isModLoader) {
            onSelectModLoader(version);
          } else {
            onSelectOptiFine?.(version);
          }
          setSelectedId(title);
        },
      };
    },
    [primaryColor, t, onSelectModLoader, onSelectOptiFine]
  );

  // Every request gets a fresh token and AbortSignal: bumping the token and
  // aborting the controller when the type changes makes any late answer for
  // the previous type a no-op.
  const beginVersionListRequest = useCallback(() => {
    requestAbortRef.current?.abort();
    const controller = new AbortController();
    requestAbortRef.current = controller;
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setIsLoading(true);
    return { requestId, signal: controller.signal };
  }, []);

  const handleFetchModLoaderVersionList = useCallback(
    (type: ModLoaderType) => {
      const { requestId, signal } = beginVersionListRequest();
      ResourceService.fetchModLoaderVersionList(selectedGameVersion.id, type)
        .then((res) => {
          if (signal.aborted || requestId !== requestIdRef.current) return;
          if (res.status === "success") {
            setVersionList(
              res.data
                .map((loader) => ({
                  ...loader,
                  description:
                    loader.description &&
                    t("LoaderSelector.releaseDate", {
                      date: ISOToDatetime(loader.description),
                    }),
                }))
                .map(buildOptionItems)
            );
          } else {
            setVersionList([]);
            toast({
              status: "error",
              title: res.message,
              description: res.details,
            });
          }
        })
        .finally(() => {
          if (signal.aborted || requestId !== requestIdRef.current) return;
          setIsLoading(false);
        });
    },
    [
      selectedGameVersion.id,
      buildOptionItems,
      t,
      toast,
      beginVersionListRequest,
    ]
  );

  const handleFetchOptiFineVersionList = useCallback(() => {
    const { requestId, signal } = beginVersionListRequest();
    ResourceService.fetchOptiFineVersionList(selectedGameVersion.id)
      .then((res) => {
        if (signal.aborted || requestId !== requestIdRef.current) return;
        if (res.status === "success") {
          setVersionList(res.data.map(buildOptionItems));
        } else {
          setVersionList([]);
          toast({
            status: "error",
            title: res.message,
            description: res.details,
          });
        }
      })
      .finally(() => {
        if (signal.aborted || requestId !== requestIdRef.current) return;
        setIsLoading(false);
      });
  }, [
    selectedGameVersion.id,
    buildOptionItems,
    toast,
    beginVersionListRequest,
  ]);

  let selectableCardItems = modLoaderTypes
    .filter(
      (it) =>
        it != ModLoaderType.Cleanroom || selectedGameVersion.id === "1.12.2"
    )
    .map(
      (type): SelectableCardProps => ({
        title: type,
        iconSrc: `/images/icons/${modLoaderTypesToIcon[type]}`,
        description:
          selectedModLoader.loaderType !== ModLoaderType.Unknown
            ? selectedModLoader.loaderType === type
              ? selectedModLoader.version ||
                t("LoaderSelector.noVersionSelected")
              : t("LoaderSelector.notCompatibleWith", {
                  item: selectedModLoader.loaderType,
                })
            : t("LoaderSelector.noVersionSelected"),
        displayMode: "selector",
        isLoading,
        // A version list is in flight: the type selector stays clickable while
        // only the leading spinner is disabled, so a slow server cannot freeze
        // the picker; the Cancel button keeps following the loading state.
        isBusy: isLoading && type === selectedType,
        isSelected: type === selectedModLoader.loaderType,
        isChevronShown: selectedType !== type,
        onSelect: () => {
          setSelectedType(type);
          if (selectedModLoader.loaderType !== type) {
            onSelectModLoader({
              loaderType: type,
              version: "",
              description: "",
            });
            setSelectedId("");
          } else {
            setSelectedId(selectedModLoader.version);
          }
          if (
            type !== ModLoaderType.Forge ||
            (selectedOptiFine && !selectedOptiFine.filename)
          ) {
            // When OptiFine is not compatible with the selected mod loader, or selected without a version, clear it
            onSelectOptiFine?.(undefined);
          }
        },
        onCancel: () => {
          if (selectedType === type) {
            if (!!selectedOptiFine) {
              setSelectedType("OptiFine");
              setSelectedId(selectedOptiFine!.filename);
            } else {
              setSelectedType(ModLoaderType.Unknown);
              setSelectedId("");
            }
          }
          onSelectModLoader(defaultModLoaderResourceInfo);
        },
      })
    );

  if (typeof onSelectOptiFine === "function") {
    selectableCardItems.push({
      title: "OptiFine",
      iconSrc: "/images/icons/OptiFine.png",
      description: selectedOptiFine
        ? selectedOptiFine.type + " " + selectedOptiFine.patch
        : selectedModLoader.loaderType === ModLoaderType.Forge ||
            selectedModLoader.loaderType === ModLoaderType.Unknown
          ? t("LoaderSelector.noVersionSelected")
          : t("LoaderSelector.notCompatibleWith", {
              item: selectedModLoader.loaderType,
            }),
      displayMode: "selector",
      isLoading,
      isBusy: isLoading && selectedType === "OptiFine",
      isSelected: !!selectedOptiFine,
      isDisabled: !(
        selectedModLoader.loaderType === ModLoaderType.Forge ||
        selectedModLoader.loaderType === ModLoaderType.Unknown
      ),
      isChevronShown: selectedType !== "OptiFine",
      onSelect: () => {
        setSelectedType("OptiFine");
        if (!selectedOptiFine) {
          onSelectOptiFine?.({ filename: "", patch: "", type: "" });
          setSelectedId("");
        } else {
          setSelectedId(selectedOptiFine.filename);
        }
        if (
          selectedModLoader.loaderType !== ModLoaderType.Unknown &&
          !selectedModLoader.version
        ) {
          onSelectModLoader(defaultModLoaderResourceInfo);
        }
      },
      onCancel: () => {
        const hasValidModLoader =
          selectedModLoader.loaderType !== ModLoaderType.Unknown &&
          !!selectedModLoader.version;

        if (selectedType === "OptiFine") {
          if (hasValidModLoader) {
            setSelectedType(selectedModLoader.loaderType);
            setSelectedId(selectedModLoader.version);
          } else {
            setSelectedType(ModLoaderType.Unknown);
            setSelectedId("");
            onSelectModLoader(defaultModLoaderResourceInfo);
          }
        }
        onSelectOptiFine?.(undefined);
      },
    });
  }

  useEffect(() => {
    if (selectedType === "OptiFine") {
      handleFetchOptiFineVersionList();
    } else if (selectedType !== ModLoaderType.Unknown) {
      handleFetchModLoaderVersionList(selectedType);
    } else {
      setVersionList([]);
    }
    // The fetch handlers only change with the game version or the translated
    // labels, which refetch elsewhere; a late response for an older type is
    // dropped by the request token, so no cleanup is needed here. Disposing
    // the selector has no warning-worthy effect either.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedType]);

  useEffect(() => () => requestAbortRef.current?.abort(), []);

  // Scroll selected item into view when version list changes or selected item changes
  useEffect(() => {
    const list = selectableCardListRef.current;
    if (!list) return;
    const selectedCard = list.querySelector<HTMLElement>(
      '[data-loader-selected="true"]'
    );
    if (!selectedCard) return;

    const frame = requestAnimationFrame(() => {
      selectedCard.scrollIntoView({
        block: "nearest",
        inline: "nearest",
      });
    });

    return () => cancelAnimationFrame(frame);
  }, [selectedModLoader.loaderType, selectedOptiFine?.filename, selectedType]);

  // The card body is the primary click target; the chevron / cancel buttons
  // inside SelectableCard keep their own handlers, so ignore bubbled clicks
  // from them to avoid selecting twice.
  const handleCardClick =
    (item: SelectableCardProps) =>
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (!(event.target instanceof Element)) return;
      if (event.target.closest("button")) return;
      if (item.isDisabled) return;
      item.onSelect();
    };

  const handleCardKeyDown =
    (item: SelectableCardProps) =>
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.target !== event.currentTarget) return;
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      if (item.isDisabled) return;
      item.onSelect();
    };

  return (
    <HStack {...props} w="100%" h="100%" spacing={4} overflow="hidden">
      {mode !== "optifine" && (
        <VStack
          spacing={3.5}
          h="100%"
          overflowY="auto"
          overflowX="hidden"
          flexShrink={0}
          ref={selectableCardListRef}
        >
          {selectableCardItems.map((item, index) => (
            <SelectableCard
              key={index}
              {...item}
              minW="3xs"
              w="100%"
              role="button"
              tabIndex={item.isDisabled ? -1 : 0}
              aria-pressed={item.isSelected}
              aria-disabled={item.isDisabled || undefined}
              cursor={item.isDisabled ? "not-allowed" : "pointer"}
              _focusVisible={{
                outline: "2px solid",
                outlineColor: `${primaryColor}.500`,
                outlineOffset: "2px",
              }}
              onClick={handleCardClick(item)}
              onKeyDown={handleCardKeyDown(item)}
              data-loader-selected={item.isSelected ? "true" : undefined}
            />
          ))}
        </VStack>
      )}
      <Section overflow="auto" flexGrow={1} w="100%" h="100%">
        {isLoading ? (
          <Center h="100%">
            <BeatLoader size={16} color="gray" />
          </Center>
        ) : versionList.length === 0 ? (
          <Center h="100%">
            <Empty withIcon={false} size="sm" />
          </Center>
        ) : (
          <RadioGroup value={selectedId} onChange={setSelectedId} h="100%">
            <VirtualOptionItemGroup h="100%" items={versionList} />
          </RadioGroup>
        )}
      </Section>
    </HStack>
  );
};
