import { apiDelete, apiGet, apiPost, apiPut } from './api';
import type {
  CreateEntityMentionInput,
  CreateInteractionInput,
  CreatePersonInput,
  CreateRelationshipInput,
  EntityMention,
  GraphData,
  Interaction,
  NlqResponse,
  NlqResult,
  NlqRouteMode,
  Person,
  Relationship,
} from '../types';

export async function createPerson(req: CreatePersonInput): Promise<Person> {
  return apiPost<Person>('/api/persons', req);
}

export async function updatePerson(id: string, req: CreatePersonInput): Promise<Person> {
  return apiPut<Person>(`/api/persons/${id}`, req);
}

export async function listPersons(): Promise<Person[]> {
  return apiGet<Person[]>('/api/persons');
}

export async function getPerson(id: string): Promise<Person | null> {
  return apiGet<Person | null>(`/api/persons/${id}`);
}

export async function deletePerson(id: string): Promise<void> {
  await apiDelete<{ deleted: boolean }>(`/api/persons/${id}`);
}

export async function searchPersonCandidates(mention: string): Promise<Person[]> {
  return apiGet<Person[]>(`/api/persons-search?mention=${encodeURIComponent(mention)}`);
}

export async function createRelationship(req: CreateRelationshipInput): Promise<Relationship> {
  return apiPost<Relationship>('/api/relationships', req);
}

export async function listRelationships(): Promise<Relationship[]> {
  return apiGet<Relationship[]>('/api/relationships');
}

export async function listRelationshipsByPerson(personId: string): Promise<Relationship[]> {
  return apiGet<Relationship[]>(`/api/persons/${personId}/relationships`);
}

export async function inferRelationships(): Promise<{ created: number }> {
  return apiPost<{ created: number }>('/api/relationships/infer');
}

export async function listPendingRelationships(): Promise<Relationship[]> {
  return apiGet<Relationship[]>('/api/relationships/pending');
}

export async function setRelationshipConfirmation(
  id: string,
  status: 'confirmed' | 'rejected',
): Promise<Relationship> {
  return apiPost<Relationship>(`/api/relationships/${id}/confirmation`, { status });
}

export async function createInteraction(req: CreateInteractionInput): Promise<Interaction> {
  return apiPost<Interaction>('/api/interactions', req);
}

export async function listInteractionsByPerson(personId: string): Promise<Interaction[]> {
  return apiGet<Interaction[]>(`/api/persons/${personId}/interactions`);
}

export async function createEntityMention(req: CreateEntityMentionInput): Promise<EntityMention> {
  return apiPost<EntityMention>('/api/entity-mentions', req);
}

export async function getGraphData(): Promise<GraphData> {
  return apiGet<GraphData>('/api/graph');
}

export async function naturalLanguageQuery(query: string, revealSensitive = false): Promise<NlqResult[]> {
  return apiPost<NlqResult[]>('/api/nlq', { query, revealSensitive });
}

export async function nlqMulti(
  query: string,
  revealSensitive?: boolean,
  routeMode?: NlqRouteMode,
): Promise<NlqResponse> {
  return apiPost<NlqResponse>('/api/nlq/multi', { query, revealSensitive, routeMode });
}

export async function nlqConfirm(intentType: string, data: Record<string, unknown>): Promise<unknown> {
  return apiPost('/api/nlq/confirm', { intentType, data });
}
