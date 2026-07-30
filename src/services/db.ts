import { invoke } from '@tauri-apps/api/core';
import type {
  CreateEntityMentionInput,
  CreateInteractionInput,
  CreatePersonInput,
  CreateRelationshipInput,
  EntityMention,
  GraphData,
  Interaction,
  NlqResult,
  Person,
  Relationship,
} from '../types';

export async function createPerson(req: CreatePersonInput): Promise<Person> {
  return invoke<Person>('create_person', { req });
}

export async function updatePerson(id: string, req: CreatePersonInput): Promise<Person> {
  return invoke<Person>('update_person', { id, req });
}

export async function listPersons(): Promise<Person[]> {
  return invoke<Person[]>('list_persons');
}

export async function getPerson(id: string): Promise<Person | null> {
  return invoke<Person | null>('get_person', { id });
}

export async function deletePerson(id: string): Promise<void> {
  return invoke<void>('delete_person', { id });
}

export async function searchPersonCandidates(mention: string): Promise<Person[]> {
  return invoke<Person[]>('search_person_candidates', { mention });
}

export async function createRelationship(req: CreateRelationshipInput): Promise<Relationship> {
  return invoke<Relationship>('create_relationship', { req });
}

export async function listRelationships(): Promise<Relationship[]> {
  return invoke<Relationship[]>('list_relationships');
}

export async function listRelationshipsByPerson(personId: string): Promise<Relationship[]> {
  return invoke<Relationship[]>('list_relationships_by_person', { personId });
}

export async function createInteraction(req: CreateInteractionInput): Promise<Interaction> {
  return invoke<Interaction>('create_interaction', { req });
}

export async function listInteractionsByPerson(personId: string): Promise<Interaction[]> {
  return invoke<Interaction[]>('list_interactions_by_person', { personId });
}

export async function createEntityMention(req: CreateEntityMentionInput): Promise<EntityMention> {
  return invoke<EntityMention>('create_entity_mention', { req });
}

export async function getGraphData(): Promise<GraphData> {
  return invoke<GraphData>('get_graph_data');
}

export async function naturalLanguageQuery(query: string, revealSensitive = false): Promise<NlqResult[]> {
  return invoke<NlqResult[]>('natural_language_query', {
    req: { query, revealSensitive },
  });
}
