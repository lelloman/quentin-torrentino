// Pipeline API client

import { get, post } from './client'

// Types

export interface PoolStatus {
  name: string
  active_jobs: number
  max_concurrent: number
  queued_jobs: number
  total_processed: number
  total_failed: number
}

export interface PipelineStatus {
  available: boolean
  running: boolean
  message: string
  conversion_pool?: PoolStatus
  placement_pool?: PoolStatus
  converting_tickets: string[]
  placing_tickets: string[]
}

// Progress types
export interface ProgressDetails {
  current_file: number
  total_files: number
  current_file_name: string
  percent: number
}

export interface TicketProgress {
  ticket_id: string
  phase: string
  progress?: ProgressDetails
  error?: string
}

// Process request types
export interface SourceFileRequest {
  path: string
  item_id: string
  dest_filename: string
}

export interface ProcessTicketRequest {
  source_files: SourceFileRequest[]
  dest_dir: string
  output_format?: string
  bitrate_kbps?: number
}

export interface ProcessTicketResponse {
  success: boolean
  message: string
  ticket_id: string
}

export interface ConverterConfig {
  max_parallel_conversions: number
  timeout_secs: number
  temp_dir: string
}

export interface ConverterInfo {
  available: boolean
  name: string
  supported_input_formats: string[]
  supported_output_formats: string[]
  config: ConverterConfig
}

export interface PlacerConfig {
  prefer_atomic_moves: boolean
  verify_checksums: boolean
  max_parallel_operations: number
}

export interface PlacerInfo {
  available: boolean
  name: string
  config: PlacerConfig
}

export interface FfmpegValidation {
  valid: boolean
  ffmpeg_available: boolean
  ffprobe_available: boolean
  message: string
}

// API functions

export async function getPipelineStatus(): Promise<PipelineStatus> {
  return get<PipelineStatus>('/pipeline/status')
}

export async function getConverterInfo(): Promise<ConverterInfo> {
  return get<ConverterInfo>('/pipeline/converter')
}

export async function getPlacerInfo(): Promise<PlacerInfo> {
  return get<PlacerInfo>('/pipeline/placer')
}

export async function validateFfmpeg(): Promise<FfmpegValidation> {
  return get<FfmpegValidation>('/pipeline/validate')
}

export async function processTicket(
  ticketId: string,
  request: ProcessTicketRequest
): Promise<ProcessTicketResponse> {
  return post<ProcessTicketResponse>(`/pipeline/process/${ticketId}`, request)
}

export async function getTicketProgress(ticketId: string): Promise<TicketProgress> {
  return get<TicketProgress>(`/pipeline/progress/${ticketId}`)
}

// Encoder capabilities types
export interface VideoFormatInfo {
  id: string
  name: string
  codec: string
  is_hardware: boolean
}

export interface EncoderCapabilities {
  h264_nvenc: boolean
  hevc_nvenc: boolean
  av1_nvenc: boolean
  h264_qsv: boolean
  hevc_qsv: boolean
  av1_qsv: boolean
  h264_amf: boolean
  hevc_amf: boolean
  av1_amf: boolean
  h264_videotoolbox: boolean
  hevc_videotoolbox: boolean
}

export interface EncoderCapabilitiesResponse {
  capabilities: EncoderCapabilities
  available_video_formats: VideoFormatInfo[]
  has_hardware_encoder: boolean
}

export async function getEncoderCapabilities(): Promise<EncoderCapabilitiesResponse> {
  return get<EncoderCapabilitiesResponse>('/pipeline/encoders')
}
