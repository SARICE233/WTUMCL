import {
  Box,
  Button,
  Divider,
  FormControl,
  FormLabel,
  Grid,
  HStack,
  IconButton,
  Image,
  Input,
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  ModalProps,
  Radio,
  RadioGroup,
  Spinner,
  Text,
  VStack,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { LuFolderOpen, LuRefreshCw } from "react-icons/lu";
import { AutoSizer } from "react-virtualized";
import SegmentedControl from "@/components/common/segmented";
import SkinPreview from "@/components/skin-preview";
import { useLauncherConfig } from "@/contexts/config";
import { useGlobalData } from "@/contexts/global-data";
import { useToast } from "@/contexts/toast";
import {
  PlayerType,
  PresetRole,
  SkinModel,
  TextureType,
} from "@/enums/account";
import { Player, PlayerCapeInfo } from "@/models/account";
import { AccountService } from "@/services/account";
import { base64ImgSrc } from "@/utils/string";

type SkinType = PresetRole | "default" | "upload";

interface ManageSkinModalProps extends Omit<ModalProps, "children"> {
  player: Player;
}

const ManageSkinModal: React.FC<ManageSkinModalProps> = ({
  player,
  isOpen,
  onClose,
  ...modalProps
}) => {
  const playerId = player.id;
  const isMicrosoft = player.playerType === PlayerType.Microsoft;
  const skin = player.textures.find((t) => t.textureType === TextureType.Skin);
  const cape = player.textures.find((t) => t.textureType === TextureType.Cape);

  const [selectedSkin, setSelectedSkin] = useState<SkinType>("default");
  const [uploadSkinFilePath, setUploadSkinFilePath] = useState<string>("");
  const [uploadCapeFilePath, setUploadCapeFilePath] = useState<string>("");
  const [skinModel, setSkinModel] = useState<SkinModel>(SkinModel.Default);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isCapeVisible, setIsCapeVisible] = useState<boolean>(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [capeList, setCapeList] = useState<PlayerCapeInfo[] | null>(null);
  const [activeCapeId, setActiveCapeId] = useState<string | null>(null);
  const [isCapeListLoading, setIsCapeListLoading] = useState<boolean>(false);
  const [isCapeListFailed, setIsCapeListFailed] = useState<boolean>(false);
  const [isCapeUpdating, setIsCapeUpdating] = useState<boolean>(false);
  const { t } = useTranslation();
  const { config } = useLauncherConfig();
  const { getPlayerList } = useGlobalData();
  const toast = useToast();
  const primaryColor = config.appearance.theme.primaryColor;

  const skinOptions = {
    default: {
      src: base64ImgSrc(skin?.image || ""),
      model: skin?.model || SkinModel.Default,
    },
    steve: { src: "/images/skins/steve.png", model: SkinModel.Default },
    alex: { src: "/images/skins/alex.png", model: SkinModel.Slim },
    upload: {
      src:
        uploadSkinFilePath && uploadSkinFilePath !== "dummy"
          ? convertFileSrc(uploadSkinFilePath)
          : "/images/skins/dummy.png",
      model: skinModel,
    },
  };

  const handleListPlayerCapes = async () => {
    if (!isMicrosoft) {
      return;
    }
    setIsCapeListLoading(true);
    setIsCapeListFailed(false);
    try {
      const resp = await AccountService.listPlayerCapes(playerId);
      if (resp.status === "success") {
        setCapeList(resp.data);
        setActiveCapeId(resp.data.find((item) => item.active)?.id ?? null);
      } else {
        setCapeList([]);
        setIsCapeListFailed(true);
        toast({
          title: resp.message,
          description: resp.details,
          status: "error",
        });
      }
    } catch (e: any) {
      setCapeList([]);
      setIsCapeListFailed(true);
      toast({
        title: t("ManageSkinModal.capeList.failed"),
        status: "error",
      });
      logger.error("Failed to load the cape list", e);
    } finally {
      setIsCapeListLoading(false);
    }
  };

  const isCapeListPending =
    isCapeListLoading ||
    (isMicrosoft && capeList === null && !isCapeListFailed);
  const hasSkinChange =
    selectedSkin === "upload"
      ? Boolean(uploadSkinFilePath)
      : selectedSkin !== "default" && selectedSkin !== skin?.preset;
  const hasCapeFileChange = !isMicrosoft && Boolean(uploadCapeFilePath);
  const activeCape = capeList?.find((item) => item.id === activeCapeId);
  let previewCapeSrc: string | undefined;
  if (isMicrosoft) {
    previewCapeSrc = activeCape
      ? base64ImgSrc(activeCape.image)
      : capeList === null && cape
        ? base64ImgSrc(cape.image)
        : undefined;
  } else {
    previewCapeSrc = uploadCapeFilePath
      ? convertFileSrc(uploadCapeFilePath)
      : cape
        ? base64ImgSrc(cape.image)
        : undefined;
  }

  useEffect(() => {
    setSelectedSkin(skin?.preset || "default");
  }, [skin]);

  useEffect(() => {
    if (!isOpen) {
      setSelectedSkin(skin?.preset || "default");
      setSkinModel(SkinModel.Default);
    }
  }, [isOpen, skin?.preset]);

  useEffect(() => {
    if (selectedSkin === "upload") {
      setUploadSkinFilePath("");
      setSkinModel(SkinModel.Default);
      setUploadCapeFilePath("");
    }
  }, [selectedSkin]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    setCapeList(null);
    setActiveCapeId(null);
    setIsCapeListFailed(false);
    if (isMicrosoft) {
      handleListPlayerCapes();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, playerId, isMicrosoft]);

  const handleSave = async () => {
    if (!hasSkinChange && !hasCapeFileChange) {
      return;
    }
    setIsLoading(true);
    try {
      let successMessage = "";
      if (selectedSkin === "upload") {
        const skinResp = await AccountService.updatePlayerTextureLocal(
          playerId,
          uploadSkinFilePath,
          TextureType.Skin,
          skinModel
        );
        if (skinResp.status !== "success") {
          toast({
            title: skinResp.message,
            description: skinResp.details,
            status: "error",
          });
          getPlayerList(true);
          return;
        }
        successMessage = skinResp.message;
      } else if (selectedSkin !== "default") {
        const skinResp = await AccountService.updatePlayerTexturePreset(
          playerId,
          selectedSkin
        );
        if (skinResp.status !== "success") {
          toast({
            title: skinResp.message,
            description: skinResp.details,
            status: "error",
          });
          getPlayerList(true);
          return;
        }
        successMessage = skinResp.message;
      }
      if (hasCapeFileChange) {
        const capeResp = await AccountService.updatePlayerTextureLocal(
          playerId,
          uploadCapeFilePath,
          TextureType.Cape,
          SkinModel.Default
        );
        if (capeResp.status !== "success") {
          toast({
            title: capeResp.message,
            description: capeResp.details,
            status: "error",
          });
          getPlayerList(true);
          return;
        }
        successMessage = capeResp.message;
      }
      toast({
        title: successMessage,
        status: "success",
      });
      getPlayerList(true);
      onClose();
    } finally {
      setIsLoading(false);
    }
  };

  const handleUpdatePlayerCape = async (capeId: string | null) => {
    if (isCapeUpdating || capeId === activeCapeId) {
      return;
    }
    setIsCapeUpdating(true);
    try {
      const resp = await AccountService.updatePlayerCape(playerId, capeId);
      if (resp.status === "success") {
        setActiveCapeId(capeId);
        toast({
          title: resp.message,
          status: "success",
        });
        getPlayerList(true);
      } else {
        toast({
          title: resp.message,
          description: resp.details,
          status: "error",
        });
      }
    } finally {
      setIsCapeUpdating(false);
    }
  };

  const handleUploadSkinFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: t("General.dialog.filterName.image"),
          extensions: ["png"],
        },
      ],
    });
    if (selected) {
      setUploadSkinFilePath(selected);
    }
  };

  const handleUploadCapeFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: t("General.dialog.filterName.image"),
          extensions: ["png"],
        },
      ],
    });
    if (selected) {
      setUploadCapeFilePath(selected);
    }
  };

  const capeOptions: { id: string | null; name: string; src: string }[] = [
    { id: null, name: t("ManageSkinModal.capeList.none"), src: "" },
    ...(capeList ?? []).map((item) => ({
      id: item.id,
      name: item.alias
        ? t(`ManageSkinModal.capeAlias.${item.alias}`, {
            defaultValue: item.alias,
          })
        : t("ManageSkinModal.capeList.unknown"),
      src: base64ImgSrc(item.image),
    })),
  ];

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      size={{ base: "2xl", lg: "3xl", xl: "4xl" }}
      returnFocusOnClose={false}
      {...modalProps}
    >
      <ModalOverlay />
      <ModalContent width="100%">
        <ModalHeader>{t("ManageSkinModal.skinManage")}</ModalHeader>
        <ModalCloseButton />
        <ModalBody width="100%">
          <Grid templateColumns="3fr 2fr" gap={4} h="400px" width="100%">
            <Box width="100%" height="100%">
              <AutoSizer>
                {({ height, width }) => (
                  <SkinPreview
                    skinSrc={skinOptions[selectedSkin].src}
                    skinModel={skinOptions[selectedSkin].model}
                    capeSrc={previewCapeSrc}
                    width={width}
                    height={height}
                    showControlBar
                    showSaveButton={
                      selectedSkin === "default" && Boolean(skin?.image)
                    }
                    isCapeVisible={isCapeVisible}
                    onCapeVisibilityChange={setIsCapeVisible}
                    errorMessage={errorMessage}
                    onSkinError={setErrorMessage}
                  />
                )}
              </AutoSizer>
            </Box>

            <VStack
              spacing={3}
              alignItems="flex-start"
              minWidth="100%"
              height="100%"
              overflowY="auto"
              pr={1}
            >
              <Text fontSize="md" fontWeight="semibold">
                {t("ManageSkinModal.section.skin")}
              </Text>
              <RadioGroup
                value={selectedSkin}
                onChange={(skinType: SkinType) => setSelectedSkin(skinType)}
              >
                <VStack spacing={2} alignItems="flex-start">
                  {Object.keys(skinOptions).map((key) => (
                    <Radio key={key} value={key} colorScheme={primaryColor}>
                      <Text fontSize="sm">{t(`ManageSkinModal.${key}`)}</Text>
                    </Radio>
                  ))}
                </VStack>
              </RadioGroup>
              {selectedSkin === "upload" && (
                <VStack spacing={2} alignItems="flex-start" width="100%">
                  <FormControl display="flex" gap={2} alignItems="center">
                    <FormLabel htmlFor="model" mb={0}>
                      {t("ManageSkinModal.model.label")}
                    </FormLabel>
                    <SegmentedControl
                      id="model"
                      size="sm"
                      selected={skinModel}
                      onSelectItem={(value) => setSkinModel(value as SkinModel)}
                      items={[
                        {
                          label: t("ManageSkinModal.model.default"),
                          value: SkinModel.Default,
                        },
                        {
                          label: t("ManageSkinModal.model.slim"),
                          value: SkinModel.Slim,
                        },
                      ]}
                    />
                  </FormControl>
                  <FormControl display="flex" gap={2} alignItems="center">
                    <FormLabel htmlFor="skin" mb={0} minWidth="max-content">
                      {t("ManageSkinModal.skin")}
                    </FormLabel>
                    <HStack id="skin" spacing={2} width="100%">
                      <Input
                        value={uploadSkinFilePath}
                        onChange={(e) => setUploadSkinFilePath(e.target.value)}
                        flex={1}
                        variant="filled"
                      />
                      <IconButton
                        onClick={handleUploadSkinFile}
                        variant="ghost"
                        width="100%"
                        aria-label={t("ManageSkinModal.upload")}
                        flex={0}
                      >
                        <LuFolderOpen />
                      </IconButton>
                    </HStack>
                  </FormControl>
                </VStack>
              )}
              <Divider />
              <Text fontSize="md" fontWeight="semibold">
                {t("ManageSkinModal.section.cape")}
              </Text>
              {isMicrosoft ? (
                <VStack spacing={2} alignItems="flex-start" width="100%">
                  {isCapeListPending ? (
                    <HStack spacing={2}>
                      <Spinner size="sm" />
                      <Text fontSize="sm">
                        {t("ManageSkinModal.capeList.loading")}
                      </Text>
                    </HStack>
                  ) : isCapeListFailed ? (
                    <HStack spacing={2}>
                      <Text fontSize="sm" color="gray.500">
                        {t("ManageSkinModal.capeList.failed")}
                      </Text>
                      <IconButton
                        size="sm"
                        variant="ghost"
                        aria-label={t("General.refresh")}
                        onClick={handleListPlayerCapes}
                      >
                        <LuRefreshCw />
                      </IconButton>
                    </HStack>
                  ) : (
                    capeList && (
                      <VStack spacing={2} alignItems="flex-start" width="100%">
                        {capeList.length === 0 && (
                          <Text fontSize="sm" color="gray.500">
                            {t("ManageSkinModal.capeList.empty")}
                          </Text>
                        )}
                        <Grid
                          templateColumns="repeat(2, 1fr)"
                          gap={2}
                          width="100%"
                        >
                          {capeOptions.map((capeOption) => (
                            <Box
                              key={capeOption.id ?? "none"}
                              as="button"
                              aria-pressed={capeOption.id === activeCapeId}
                              borderWidth={2}
                              borderColor={
                                capeOption.id === activeCapeId
                                  ? `${primaryColor}.500`
                                  : "transparent"
                              }
                              borderRadius="md"
                              p={2}
                              textAlign="center"
                              cursor={
                                isCapeUpdating ? "not-allowed" : "pointer"
                              }
                              opacity={isCapeUpdating ? 0.6 : 1}
                              onClick={() =>
                                handleUpdatePlayerCape(capeOption.id)
                              }
                              _hover={{ opacity: 0.85 }}
                              _focusVisible={{
                                outline: "2px solid",
                                outlineColor: `${primaryColor}.500`,
                                outlineOffset: "2px",
                              }}
                            >
                              <VStack spacing={1}>
                                {capeOption.src ? (
                                  <Image
                                    src={capeOption.src}
                                    alt={capeOption.name}
                                    boxSize="56px"
                                    objectFit="contain"
                                  />
                                ) : (
                                  <Box
                                    boxSize="56px"
                                    borderWidth={1}
                                    borderStyle="dashed"
                                    borderRadius="sm"
                                  />
                                )}
                                <Text
                                  fontSize="xs"
                                  fontWeight={
                                    capeOption.id === activeCapeId
                                      ? "semibold"
                                      : "normal"
                                  }
                                  noOfLines={1}
                                  width="100%"
                                >
                                  {capeOption.name}
                                </Text>
                              </VStack>
                            </Box>
                          ))}
                        </Grid>
                      </VStack>
                    )
                  )}
                </VStack>
              ) : (
                <FormControl display="flex" gap={2} alignItems="center">
                  <FormLabel htmlFor="cape" mb={0} minWidth="max-content">
                    {t("ManageSkinModal.cape")}
                  </FormLabel>
                  <HStack id="cape" spacing={2} width="100%">
                    <Input
                      value={uploadCapeFilePath}
                      onChange={(e) => setUploadCapeFilePath(e.target.value)}
                      flex={1}
                      variant="filled"
                      disabled={!isCapeVisible}
                    />
                    <IconButton
                      onClick={handleUploadCapeFile}
                      variant="ghost"
                      width="100%"
                      aria-label={t("ManageSkinModal.upload")}
                      flex={0}
                      disabled={!isCapeVisible}
                    >
                      <LuFolderOpen />
                    </IconButton>
                  </HStack>
                </FormControl>
              )}
            </VStack>
          </Grid>
        </ModalBody>

        <ModalFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("General.cancel")}
          </Button>
          <Button
            variant="solid"
            colorScheme={primaryColor}
            onClick={handleSave}
            isLoading={isLoading}
            disabled={
              errorMessage !== null || (!hasSkinChange && !hasCapeFileChange)
            }
          >
            {t("General.confirm")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
};

export default ManageSkinModal;
