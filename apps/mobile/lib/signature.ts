import * as ImagePicker from "expo-image-picker";
import * as ImageManipulator from "expo-image-manipulator";

const MAX_WIDTH = 1200;
const JPEG_QUALITY = 0.75;

/**
 * Prompts for a signature image (camera or library), downsizes to
 * MAX_WIDTH, and returns base64 JPEG.
 */
export async function captureSignature(): Promise<string | null> {
  const perm = await ImagePicker.requestCameraPermissionsAsync();
  const lib = await ImagePicker.requestMediaLibraryPermissionsAsync();
  if (!perm.granted && !lib.granted) {
    throw new Error("camera and library permission both denied");
  }

  const result = await ImagePicker.launchCameraAsync({
    mediaTypes: ImagePicker.MediaTypeOptions.Images,
    quality: 1,
    allowsEditing: false,
  });

  if (result.canceled || !result.assets.length) return null;
  const uri = result.assets[0].uri;

  const manipulated = await ImageManipulator.manipulateAsync(
    uri,
    [{ resize: { width: MAX_WIDTH } }],
    { compress: JPEG_QUALITY, format: ImageManipulator.SaveFormat.JPEG, base64: true }
  );

  if (!manipulated.base64) throw new Error("no base64 returned from manipulator");
  return manipulated.base64;
}
